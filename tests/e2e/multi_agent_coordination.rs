//! End-to-end tests for multi-agent coordination scenarios
//!
//! These tests simulate real-world multi-agent scenarios to validate
//! the complete system functionality.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, sleep};
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus},
    issue::{Issue, IssueStatus, IssuePriority},
    message::{Message, MessageType},
    knowledge::{Knowledge, KnowledgeType, AccessLevel},
};
use vibe_ensemble_storage::StorageManager;

use crate::common::{
    database::DatabaseTestHelper,
    fixtures::{TestScenarios, TestDataFactory},
    agents::{AgentNetwork, AgentNetworkBuilder, MockAgent},
    assertions::{AgentAssertions, IssueAssertions, MessageAssertions},
};

/// Tests complete development workflow with multiple agents
#[tokio::test]
async fn test_development_workflow_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create development team scenario
    let scenario = TestScenarios::development_team();
    
    // Register agents in storage
    for agent in scenario.all_agents() {
        storage_manager.agents().create_agent(agent.clone()).await.unwrap();
    }
    
    // Create project issues
    let issues = TestScenarios::issue_backlog();
    let mut issue_ids = Vec::new();
    
    for issue in issues {
        let issue_id = storage_manager.issues().create_issue(issue).await.unwrap();
        issue_ids.push(issue_id);
    }
    
    // Simulate coordinator assigning issues
    let coordinator_id = scenario.coordinator.id();
    let backend_id = scenario.backend_dev.id();
    let frontend_id = scenario.frontend_dev.id();
    let qa_id = scenario.qa_agent.id();
    
    // Assign issues to appropriate agents
    storage_manager.issues()
        .assign_issue(issue_ids[0], backend_id)  // Auth system -> Backend dev
        .await.unwrap();
    
    storage_manager.issues()
        .assign_issue(issue_ids[1], backend_id)  // Messaging -> Backend dev
        .await.unwrap();
    
    storage_manager.issues()
        .assign_issue(issue_ids[2], backend_id)  // DB optimization -> Backend dev
        .await.unwrap();
    
    // Simulate agents starting work
    storage_manager.issues()
        .update_status(issue_ids[0], IssueStatus::InProgress)
        .await.unwrap();
    
    // Create knowledge entries as work progresses
    let auth_knowledge = Knowledge::builder()
        .title("JWT Authentication Implementation Guide")
        .content("Step-by-step guide for implementing secure JWT authentication...")
        .knowledge_type(KnowledgeType::TechnicalDocumentation)
        .access_level(AccessLevel::TeamVisible)
        .created_by(backend_id)
        .build()
        .unwrap();
    
    storage_manager.knowledge().create_knowledge(auth_knowledge).await.unwrap();
    
    // Simulate message exchanges
    let progress_message = Message::broadcast(
        backend_id,
        "Authentication system implementation in progress. JWT tokens working correctly."
    ).unwrap();
    
    storage_manager.messages().create_message(progress_message).await.unwrap();
    
    // Simulate issue completion
    storage_manager.issues()
        .update_status(issue_ids[0], IssueStatus::InReview)
        .await.unwrap();
    
    // QA agent reviews the work
    let review_message = Message::direct(
        qa_id,
        backend_id,
        "Authentication system looks good. Running security tests now."
    ).unwrap();
    
    storage_manager.messages().create_message(review_message).await.unwrap();
    
    // Complete the issue
    storage_manager.issues()
        .update_status(issue_ids[0], IssueStatus::Done)
        .await.unwrap();
    
    // Verify workflow completion
    let completed_issue = storage_manager.issues()
        .get_issue(issue_ids[0])
        .await.unwrap();
    
    assert_eq!(completed_issue.status(), IssueStatus::Done);
    assert!(completed_issue.assigned_to().is_some());
    
    // Verify knowledge was created
    let all_knowledge = storage_manager.knowledge()
        .search_knowledge("JWT".to_string(), backend_id)
        .await.unwrap();
    
    assert!(!all_knowledge.is_empty());
    
    // Verify communication occurred
    let messages = storage_manager.messages()
        .get_recent_messages(10)
        .await.unwrap();
    
    assert!(messages.len() >= 2);
}

/// Tests agent failure and recovery scenarios
#[tokio::test]
async fn test_agent_failure_recovery_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create agent network with coordinator and workers
    let network = AgentNetworkBuilder::new()
        .with_coordinator("coordinator")
        .with_workers(3, "worker", vec!["coding".to_string(), "testing".to_string()])
        .build();
    
    // Register all agents
    for agent in network.all_agents() {
        storage_manager.agents().create_agent(agent.agent().clone()).await.unwrap();
    }
    
    // Create test issues
    let mut issue_ids = Vec::new();
    for i in 0..5 {
        let issue = Issue::builder()
            .title(format!("Task {}", i))
            .description("Test task for failure scenario")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();
        
        let issue_id = storage_manager.issues().create_issue(issue).await.unwrap();
        issue_ids.push(issue_id);
    }
    
    // Assign issues to workers
    let workers: Vec<_> = network.all_agents()
        .into_iter()
        .filter(|agent| agent.name().starts_with("worker"))
        .collect();
    
    for (i, issue_id) in issue_ids.iter().enumerate() {
        let worker = &workers[i % workers.len()];
        storage_manager.issues()
            .assign_issue(*issue_id, worker.id())
            .await.unwrap();
        
        storage_manager.issues()
            .update_status(*issue_id, IssueStatus::InProgress)
            .await.unwrap();
    }
    
    // Simulate worker failure
    let failed_worker = &workers[0];
    failed_worker.go_offline().await;
    
    // Update agent status to reflect failure
    storage_manager.agents()
        .update_agent_status(failed_worker.id(), AgentStatus::Inactive)
        .await.unwrap();
    
    // Simulate coordinator detecting failure and reassigning work
    let failed_worker_issues = storage_manager.issues()
        .get_issues_by_assignee(failed_worker.id())
        .await.unwrap();
    
    // Reassign failed worker's issues to healthy workers
    for issue in failed_worker_issues {
        if issue.status() != IssueStatus::Done {
            let healthy_worker = &workers[1]; // Use second worker
            storage_manager.issues()
                .assign_issue(issue.id(), healthy_worker.id())
                .await.unwrap();
        }
    }
    
    // Simulate failed worker coming back online
    sleep(Duration::from_millis(100)).await;
    failed_worker.go_online().await;
    
    storage_manager.agents()
        .update_agent_status(failed_worker.id(), AgentStatus::Active)
        .await.unwrap();
    
    // Verify system recovery
    let all_agents = storage_manager.agents().list_agents().await.unwrap();
    let active_agents = all_agents.iter()
        .filter(|a| a.status() == AgentStatus::Active)
        .count();
    
    assert_eq!(active_agents, 4); // 1 coordinator + 3 workers
    
    // Verify all issues are still assigned
    for issue_id in issue_ids {
        let issue = storage_manager.issues().get_issue(issue_id).await.unwrap();
        assert!(issue.assigned_to().is_some());
    }
}

/// Tests knowledge sharing and collaboration scenarios
#[tokio::test]
async fn test_knowledge_collaboration_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create specialized agents
    let agents = vec![
        MockAgent::with_capabilities("backend-expert", vec!["rust".to_string(), "databases".to_string()]),
        MockAgent::with_capabilities("frontend-expert", vec!["react".to_string(), "typescript".to_string()]),
        MockAgent::with_capabilities("devops-expert", vec!["docker".to_string(), "kubernetes".to_string()]),
    ];
    
    // Register agents
    for agent in &agents {
        storage_manager.agents().create_agent(agent.agent().clone()).await.unwrap();
    }
    
    // Each agent contributes knowledge in their domain
    let backend_knowledge = Knowledge::builder()
        .title("Rust async patterns for web servers")
        .content("Best practices for using async/await in Rust web applications...")
        .knowledge_type(KnowledgeType::BestPractice)
        .access_level(AccessLevel::TeamVisible)
        .created_by(agents[0].id())
        .tags(vec!["rust".to_string(), "async".to_string(), "web".to_string()])
        .build()
        .unwrap();
    
    let frontend_knowledge = Knowledge::builder()
        .title("React component testing strategies")
        .content("Comprehensive guide to testing React components with Jest and Testing Library...")
        .knowledge_type(KnowledgeType::TechnicalDocumentation)
        .access_level(AccessLevel::TeamVisible)
        .created_by(agents[1].id())
        .tags(vec!["react".to_string(), "testing".to_string(), "frontend".to_string()])
        .build()
        .unwrap();
    
    let devops_knowledge = Knowledge::builder()
        .title("Container orchestration troubleshooting")
        .content("Common Kubernetes issues and their solutions...")
        .knowledge_type(KnowledgeType::TroubleshootingGuide)
        .access_level(AccessLevel::TeamVisible)
        .created_by(agents[2].id())
        .tags(vec!["kubernetes".to_string(), "docker".to_string(), "troubleshooting".to_string()])
        .build()
        .unwrap();
    
    // Add knowledge to repository
    storage_manager.knowledge().create_knowledge(backend_knowledge).await.unwrap();
    storage_manager.knowledge().create_knowledge(frontend_knowledge).await.unwrap();
    storage_manager.knowledge().create_knowledge(devops_knowledge).await.unwrap();
    
    // Simulate agents searching for knowledge
    let rust_results = storage_manager.knowledge()
        .search_knowledge("rust".to_string(), agents[1].id())
        .await.unwrap();
    
    assert!(!rust_results.is_empty());
    assert!(rust_results.iter().any(|k| k.title().contains("Rust")));
    
    // Simulate collaborative discussion about knowledge
    let discussion_messages = vec![
        Message::broadcast(agents[1].id(), "Found great Rust async patterns in the knowledge base. Very helpful for our backend work!").unwrap(),
        Message::direct(agents[0].id(), agents[1].id(), "Glad it's useful! Let me know if you need clarification on any patterns.").unwrap(),
        Message::broadcast(agents[2].id(), "Added Kubernetes troubleshooting guide. Should help with deployment issues.").unwrap(),
    ];
    
    for message in discussion_messages {
        storage_manager.messages().create_message(message).await.unwrap();
    }
    
    // Verify cross-domain knowledge access
    let all_knowledge = storage_manager.knowledge()
        .get_all_accessible_knowledge(agents[0].id())
        .await.unwrap();
    
    assert!(all_knowledge.len() >= 3);
    
    // Verify different knowledge types are available
    let knowledge_types: std::collections::HashSet<_> = all_knowledge
        .iter()
        .map(|k| k.knowledge_type())
        .collect();
    
    assert!(knowledge_types.len() > 1);
}

/// Tests system scaling with increasing load
#[tokio::test]
async fn test_system_scaling_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create load test data
    let load_data = TestDataFactory::create_load_test_data(20, 100, 200);
    
    // Measure registration time
    let start_time = std::time::Instant::now();
    
    // Register all agents
    for agent in &load_data.agents {
        storage_manager.agents().create_agent(agent.clone()).await.unwrap();
    }
    
    let agent_registration_time = start_time.elapsed();
    println!("Agent registration time: {:?}", agent_registration_time);
    
    // Create issues
    let issue_start = std::time::Instant::now();
    let mut issue_ids = Vec::new();
    
    for issue in &load_data.issues {
        let issue_id = storage_manager.issues().create_issue(issue.clone()).await.unwrap();
        issue_ids.push(issue_id);
    }
    
    let issue_creation_time = issue_start.elapsed();
    println!("Issue creation time: {:?}", issue_creation_time);
    
    // Send messages
    let message_start = std::time::Instant::now();
    
    for message in &load_data.messages {
        storage_manager.messages().create_message(message.clone()).await.unwrap();
    }
    
    let message_creation_time = message_start.elapsed();
    println!("Message creation time: {:?}", message_creation_time);
    
    // Verify data integrity
    let agent_count = storage_manager.agents().count_agents().await.unwrap();
    assert_eq!(agent_count, load_data.agents.len());
    
    let issue_count = storage_manager.issues().count_issues().await.unwrap();
    assert_eq!(issue_count, load_data.issues.len());
    
    let message_count = storage_manager.messages().count_messages().await.unwrap();
    assert_eq!(message_count, load_data.messages.len());
    
    // Performance assertions
    assert!(agent_registration_time < Duration::from_secs(5));
    assert!(issue_creation_time < Duration::from_secs(10));
    assert!(message_creation_time < Duration::from_secs(5));
}

/// Tests real-time coordination between agents
#[tokio::test]
async fn test_realtime_coordination_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create network of agents
    let network = AgentNetworkBuilder::new()
        .with_coordinator("coordinator")
        .with_workers(2, "worker", vec!["collaboration".to_string()])
        .build();
    
    // Register agents
    for agent in network.all_agents() {
        storage_manager.agents().create_agent(agent.agent().clone()).await.unwrap();
    }
    
    // Create shared task
    let shared_task = Issue::builder()
        .title("Implement collaborative feature")
        .description("Feature requiring coordination between multiple agents")
        .priority(IssuePriority::High)
        .build()
        .unwrap();
    
    let task_id = storage_manager.issues().create_issue(shared_task).await.unwrap();
    
    // Simulate real-time coordination
    let agents: Vec<_> = network.all_agents().collect();
    let coordinator = network.get_agent_by_name("coordinator").unwrap();
    let worker1 = network.get_agent_by_name("worker-0").unwrap();
    let worker2 = network.get_agent_by_name("worker-1").unwrap();
    
    // Coordinator initiates coordination
    let init_message = Message::broadcast(
        coordinator.id(),
        "Starting collaborative task implementation. Workers please coordinate on components."
    ).unwrap();
    
    storage_manager.messages().create_message(init_message).await.unwrap();
    
    // Workers coordinate between themselves
    network.deliver_message(
        worker1.id(),
        Some(worker2.id()),
        "I'll handle the backend API. Can you work on the frontend integration?"
    ).await;
    
    network.deliver_message(
        worker2.id(),
        Some(worker1.id()),
        "Confirmed. Starting frontend work now. Will sync with you on the interface."
    ).await;
    
    // Simulate progress updates
    let progress_updates = vec![
        (worker1.id(), "Backend API 50% complete"),
        (worker2.id(), "Frontend integration 30% complete"),
        (worker1.id(), "API testing complete, ready for integration"),
        (worker2.id(), "Frontend ready for API integration testing"),
    ];
    
    for (sender_id, message) in progress_updates {
        let update_message = Message::broadcast(sender_id, message).unwrap();
        storage_manager.messages().create_message(update_message).await.unwrap();
    }
    
    // Final coordination
    network.deliver_message(
        coordinator.id(),
        None,
        "Great progress team! Integration testing phase starting."
    ).await;
    
    // Complete task
    storage_manager.issues()
        .update_status(task_id, IssueStatus::Done)
        .await.unwrap();
    
    // Verify coordination was successful
    let final_messages = storage_manager.messages()
        .get_recent_messages(20)
        .await.unwrap();
    
    assert!(final_messages.len() >= 6);
    
    let completed_task = storage_manager.issues()
        .get_issue(task_id)
        .await.unwrap();
    
    assert_eq!(completed_task.status(), IssueStatus::Done);
    
    // Verify message flow included all participants
    let participants: std::collections::HashSet<_> = final_messages
        .iter()
        .map(|m| m.sender_id())
        .collect();
    
    assert!(participants.len() >= 2);
}

/// Tests error recovery in multi-agent scenarios
#[tokio::test]
async fn test_error_recovery_e2e() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Create agent network
    let network = AgentNetworkBuilder::new()
        .with_coordinator("coordinator")
        .with_workers(3, "worker", vec!["error-handling".to_string()])
        .build();
    
    for agent in network.all_agents() {
        storage_manager.agents().create_agent(agent.agent().clone()).await.unwrap();
    }
    
    // Create critical task
    let critical_task = Issue::builder()
        .title("Critical system update")
        .description("Important task that cannot fail")
        .priority(IssuePriority::Critical)
        .build()
        .unwrap();
    
    let task_id = storage_manager.issues().create_issue(critical_task).await.unwrap();
    
    // Assign to primary worker
    let primary_worker = network.get_agent_by_name("worker-0").unwrap();
    storage_manager.issues()
        .assign_issue(task_id, primary_worker.id())
        .await.unwrap();
    
    storage_manager.issues()
        .update_status(task_id, IssueStatus::InProgress)
        .await.unwrap();
    
    // Simulate worker encountering error
    primary_worker.go_offline().await;
    
    // Simulate coordinator detecting problem
    sleep(Duration::from_millis(100)).await;
    
    let backup_worker = network.get_agent_by_name("worker-1").unwrap();
    
    // Reassign task
    storage_manager.issues()
        .assign_issue(task_id, backup_worker.id())
        .await.unwrap();
    
    // Log the recovery
    let recovery_message = Message::broadcast(
        network.get_agent_by_name("coordinator").unwrap().id(),
        "Task reassigned due to worker failure. Backup worker taking over."
    ).unwrap();
    
    storage_manager.messages().create_message(recovery_message).await.unwrap();
    
    // Complete task with backup worker
    storage_manager.issues()
        .update_status(task_id, IssueStatus::Done)
        .await.unwrap();
    
    // Verify recovery was successful
    let recovered_task = storage_manager.issues()
        .get_issue(task_id)
        .await.unwrap();
    
    assert_eq!(recovered_task.status(), IssueStatus::Done);
    assert_eq!(recovered_task.assigned_to().unwrap(), backup_worker.id());
    
    // Verify recovery was logged
    let messages = storage_manager.messages()
        .get_recent_messages(10)
        .await.unwrap();
    
    assert!(messages.iter().any(|m| m.content().contains("reassigned")));
}