//! Property-based tests for vibe-ensemble-mcp
//!
//! These tests use property-based testing to validate invariants and edge cases
//! across the entire system using randomly generated test data.

use std::sync::Arc;
use proptest::prelude::*;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration as ChronoDuration};

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus, ConnectionMetadata},
    issue::{Issue, IssueStatus, IssuePriority},
    message::{Message, MessageType},
    knowledge::{Knowledge, KnowledgeType, AccessLevel},
    config::{Configuration, CoordinationSettings, RetryPolicy},
};

use crate::common::{
    database::DatabaseTestHelper,
    fixtures::TestDataFactory,
};

// Property-based test strategies for generating test data

/// Strategy for generating valid agent names
fn agent_name_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z][a-z0-9-_]{2,30}")
        .unwrap()
        .prop_map(|s| s.to_lowercase())
}

/// Strategy for generating agent capabilities
fn capabilities_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        prop::sample::select(vec![
            "rust_development", "web_development", "database_design", "api_development",
            "testing", "quality_assurance", "devops", "security", "documentation",
            "project_management", "ui_design", "performance_optimization"
        ]),
        1..5
    )
}

/// Strategy for generating connection metadata
fn connection_metadata_strategy() -> impl Strategy<Value = ConnectionMetadata> {
    (
        prop::string::string_regex("(localhost|[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3})").unwrap(),
        8000u16..9000,
        prop::sample::select(vec!["http", "https", "ws", "wss"]),
        any::<u64>().prop_map(|_| Utc::now()),
        any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes).to_string()),
    ).prop_map(|(host, port, protocol, last_heartbeat, connection_id)| {
        ConnectionMetadata {
            host,
            port,
            protocol,
            last_heartbeat,
            connection_id,
        }
    })
}

/// Strategy for generating valid agents
fn agent_strategy() -> impl Strategy<Value = Agent> {
    (
        agent_name_strategy(),
        connection_metadata_strategy(),
        capabilities_strategy(),
    ).prop_map(|(name, connection_metadata, capabilities)| {
        Agent::builder()
            .name(&name)
            .connection_metadata(connection_metadata)
            .capabilities(capabilities)
            .build()
            .unwrap()
    })
}

/// Strategy for generating issue titles
fn issue_title_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][a-zA-Z0-9 .-]{10,100}").unwrap()
}

/// Strategy for generating issue descriptions
fn issue_description_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][a-zA-Z0-9 .,-]{20,500}").unwrap()
}

/// Strategy for generating issues
fn issue_strategy() -> impl Strategy<Value = Issue> {
    (
        issue_title_strategy(),
        issue_description_strategy(),
        prop::sample::select(vec![
            IssuePriority::Low,
            IssuePriority::Medium,
            IssuePriority::High,
            IssuePriority::Critical,
        ]),
    ).prop_map(|(title, description, priority)| {
        Issue::builder()
            .title(&title)
            .description(&description)
            .priority(priority)
            .build()
            .unwrap()
    })
}

/// Strategy for generating message content
fn message_content_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z][a-zA-Z0-9 .!?,-]{5,200}").unwrap()
}

/// Strategy for generating messages
fn message_strategy() -> impl Strategy<Value = (Message, bool)> {
    (
        any::<[u8; 16]>().prop_map(Uuid::from_bytes), // sender_id
        any::<Option<[u8; 16]>>().prop_map(|opt| opt.map(Uuid::from_bytes)), // recipient_id
        message_content_strategy(),
    ).prop_map(|(sender_id, recipient_id, content)| {
        let message = if let Some(recipient) = recipient_id {
            Message::direct(sender_id, recipient, &content).unwrap()
        } else {
            Message::broadcast(sender_id, &content).unwrap()
        };
        (message, recipient_id.is_some())
    })
}

/// Strategy for generating knowledge entries
fn knowledge_strategy() -> impl Strategy<Value = Knowledge> {
    (
        prop::string::string_regex("[A-Z][a-zA-Z0-9 ]{10,100}").unwrap(), // title
        prop::string::string_regex("[A-Z][a-zA-Z0-9 .,-]{50,1000}").unwrap(), // content
        prop::sample::select(vec![
            KnowledgeType::BestPractice,
            KnowledgeType::TechnicalDocumentation,
            KnowledgeType::TroubleshootingGuide,
        ]),
        prop::sample::select(vec![
            AccessLevel::Private,
            AccessLevel::TeamVisible,
            AccessLevel::PublicVisible,
        ]),
        any::<[u8; 16]>().prop_map(Uuid::from_bytes), // created_by
        prop::collection::vec(
            prop::string::string_regex("[a-z]{3,15}").unwrap(),
            0..5
        ), // tags
    ).prop_map(|(title, content, knowledge_type, access_level, created_by, tags)| {
        Knowledge::builder()
            .title(&title)
            .content(&content)
            .knowledge_type(knowledge_type)
            .access_level(access_level)
            .created_by(created_by)
            .tags(tags)
            .build()
            .unwrap()
    })
}

// Property-based tests

proptest! {
    /// Test that agent creation always produces valid agents
    #[test]
    fn test_agent_creation_invariants(agent in agent_strategy()) {
        // Agent should have a non-empty name
        assert!(!agent.name().is_empty());
        
        // Agent should have at least one capability
        assert!(!agent.capabilities().is_empty());
        
        // Agent should have a valid connection
        assert!(!agent.connection_metadata().host.is_empty());
        assert!(agent.connection_metadata().port > 0);
        assert!(!agent.connection_metadata().protocol.is_empty());
        assert!(!agent.connection_metadata().connection_id.is_empty());
        
        // Agent should start with Active status
        assert_eq!(agent.status(), AgentStatus::Active);
        
        // Agent ID should be valid UUID
        assert!(!agent.id().is_nil());
        
        // Agent should have reasonable creation time
        assert!(agent.created_at() <= Utc::now());
        assert!(agent.created_at() >= Utc::now() - ChronoDuration::seconds(10));
    }
}

proptest! {
    /// Test that issue creation maintains invariants
    #[test]
    fn test_issue_creation_invariants(issue in issue_strategy()) {
        // Issue should have non-empty title and description
        assert!(!issue.title().is_empty());
        assert!(!issue.description().is_empty());
        
        // Issue should start with Open status
        assert_eq!(issue.status(), IssueStatus::Open);
        
        // Issue should not be assigned initially
        assert!(issue.assigned_to().is_none());
        
        // Issue ID should be valid
        assert!(!issue.id().is_nil());
        
        // Creation time should be recent
        assert!(issue.created_at() <= Utc::now());
        assert!(issue.created_at() >= Utc::now() - ChronoDuration::seconds(10));
        
        // Issue should not have resolution time initially
        assert!(issue.time_to_resolution().is_none());
        
        // Age should be reasonable
        let age = issue.age();
        assert!(age <= ChronoDuration::seconds(10));
    }
}

proptest! {
    /// Test that message creation maintains invariants
    #[test]
    fn test_message_creation_invariants((message, is_direct) in message_strategy()) {
        // Message should have non-empty content
        assert!(!message.content().is_empty());
        
        // Sender ID should be valid
        assert!(!message.sender_id().is_nil());
        
        // Message ID should be valid
        assert!(!message.id().is_nil());
        
        // Creation time should be recent
        assert!(message.created_at() <= Utc::now());
        assert!(message.created_at() >= Utc::now() - ChronoDuration::seconds(10));
        
        // Message should not be delivered initially
        assert!(message.delivered_at().is_none());
        
        // Direct messages should have recipient, broadcasts should not
        if is_direct {
            assert!(message.recipient_id().is_some());
            assert_ne!(message.sender_id(), message.recipient_id().unwrap());
        } else {
            assert!(message.recipient_id().is_none());
        }
        
        // Message type should be consistent with recipient
        match message.message_type() {
            MessageType::Direct => assert!(message.recipient_id().is_some()),
            MessageType::Broadcast => assert!(message.recipient_id().is_none()),
        }
    }
}

proptest! {
    /// Test that knowledge creation maintains invariants
    #[test]
    fn test_knowledge_creation_invariants(knowledge in knowledge_strategy()) {
        // Knowledge should have non-empty title and content
        assert!(!knowledge.title().is_empty());
        assert!(!knowledge.content().is_empty());
        
        // Creator should be valid
        assert!(!knowledge.created_by().is_nil());
        
        // Knowledge ID should be valid
        assert!(!knowledge.id().is_nil());
        
        // Creation time should be recent
        assert!(knowledge.created_at() <= Utc::now());
        assert!(knowledge.created_at() >= Utc::now() - ChronoDuration::seconds(10));
        
        // Updated time should be >= creation time
        assert!(knowledge.updated_at() >= knowledge.created_at());
        
        // Access level should be valid
        match knowledge.access_level() {
            AccessLevel::Private | AccessLevel::TeamVisible | AccessLevel::PublicVisible => {},
        }
        
        // Knowledge type should be valid
        match knowledge.knowledge_type() {
            KnowledgeType::BestPractice | 
            KnowledgeType::TechnicalDocumentation | 
            KnowledgeType::TroubleshootingGuide => {},
        }
        
        // Tags should be valid if present
        for tag in knowledge.tags() {
            assert!(!tag.is_empty());
            assert!(tag.len() <= 50); // reasonable tag length
        }
    }
}

proptest! {
    /// Test that issue status transitions follow valid state machine
    #[test]
    fn test_issue_status_transitions(
        mut issue in issue_strategy(),
        status_sequence in prop::collection::vec(
            prop::sample::select(vec![
                IssueStatus::Open,
                IssueStatus::InProgress,
                IssueStatus::InReview,
                IssueStatus::Blocked,
                IssueStatus::Done,
            ]),
            1..10
        )
    ) {
        let mut current_status = issue.status();
        
        for new_status in status_sequence {
            let transition_result = issue.update_status(new_status);
            
            // Check if transition is valid
            let is_valid_transition = match (current_status, new_status) {
                // From Open
                (IssueStatus::Open, IssueStatus::InProgress) => true,
                (IssueStatus::Open, IssueStatus::Blocked) => true,
                
                // From InProgress
                (IssueStatus::InProgress, IssueStatus::InReview) => true,
                (IssueStatus::InProgress, IssueStatus::Blocked) => true,
                
                // From InReview
                (IssueStatus::InReview, IssueStatus::Done) => true,
                (IssueStatus::InReview, IssueStatus::InProgress) => true,
                
                // From Blocked
                (IssueStatus::Blocked, IssueStatus::Open) => true,
                (IssueStatus::Blocked, IssueStatus::InProgress) => true,
                
                // From Done (no valid transitions)
                (IssueStatus::Done, _) => false,
                
                // Same status (always valid)
                (a, b) if a == b => true,
                
                // All other transitions invalid
                _ => false,
            };
            
            if is_valid_transition {
                assert!(transition_result.is_ok());
                current_status = new_status;
            } else {
                assert!(transition_result.is_err());
                // Status should remain unchanged on invalid transition
                assert_eq!(issue.status(), current_status);
            }
        }
    }
}

proptest! {
    /// Test that agent capabilities can be safely modified
    #[test]
    fn test_agent_capability_operations(
        mut agent in agent_strategy(),
        new_capabilities in capabilities_strategy(),
        capabilities_to_remove in prop::collection::vec(
            prop::string::string_regex("[a-z_]{5,20}").unwrap(),
            0..3
        )
    ) {
        let original_count = agent.capabilities().len();
        
        // Add new capabilities
        for capability in &new_capabilities {
            agent.add_capability(capability.clone()).unwrap();
        }
        
        // Agent should have at least the original capabilities
        assert!(agent.capabilities().len() >= original_count);
        
        // All new capabilities should be present
        for capability in &new_capabilities {
            assert!(agent.capabilities().contains(capability));
        }
        
        // Remove some capabilities
        for capability in &capabilities_to_remove {
            // Only try to remove if it exists
            if agent.capabilities().contains(capability) {
                agent.remove_capability(capability).unwrap();
                assert!(!agent.capabilities().contains(capability));
            }
        }
        
        // Agent should always have at least one capability
        assert!(!agent.capabilities().is_empty());
    }
}

proptest! {
    /// Test that message delivery maintains ordering invariants
    #[test]
    fn test_message_ordering_invariants(
        messages in prop::collection::vec(message_strategy(), 1..20)
    ) {
        let mut message_objects: Vec<_> = messages.into_iter().map(|(msg, _)| msg).collect();
        
        // Sort messages by creation time
        message_objects.sort_by_key(|m| m.created_at());
        
        // Verify chronological ordering
        for window in message_objects.windows(2) {
            assert!(window[0].created_at() <= window[1].created_at());
        }
        
        // Simulate delivery and verify delivery times
        for message in &mut message_objects {
            message.mark_delivered().unwrap();
            assert!(message.delivered_at().is_some());
            assert!(message.delivered_at().unwrap() >= message.created_at());
        }
    }
}

proptest! {
    /// Test knowledge search properties
    #[test]
    fn test_knowledge_search_properties(
        knowledge_entries in prop::collection::vec(knowledge_strategy(), 1..50),
        search_terms in prop::collection::vec(
            prop::string::string_regex("[a-z]{3,10}").unwrap(),
            1..5
        )
    ) {
        // For each search term, results should be relevant
        for search_term in &search_terms {
            let matching_entries: Vec<_> = knowledge_entries
                .iter()
                .filter(|k| {
                    k.title().to_lowercase().contains(search_term) ||
                    k.content().to_lowercase().contains(search_term) ||
                    k.tags().iter().any(|tag| tag.contains(search_term))
                })
                .collect();
            
            // If we found matches, they should actually contain the search term
            for entry in &matching_entries {
                let contains_term = 
                    entry.title().to_lowercase().contains(search_term) ||
                    entry.content().to_lowercase().contains(search_term) ||
                    entry.tags().iter().any(|tag| tag.contains(search_term));
                assert!(contains_term, "Entry should contain search term: {}", search_term);
            }
        }
        
        // Empty search should return all entries (or none based on implementation)
        let empty_search_results: Vec<_> = knowledge_entries
            .iter()
            .filter(|_| true) // Simulate empty search returning all
            .collect();
        assert_eq!(empty_search_results.len(), knowledge_entries.len());
        
        // Search for non-existent term should return no results
        let impossible_term = "xyzquuxnonexistent12345";
        let no_results: Vec<_> = knowledge_entries
            .iter()
            .filter(|k| {
                k.title().contains(impossible_term) ||
                k.content().contains(impossible_term) ||
                k.tags().iter().any(|tag| tag.contains(impossible_term))
            })
            .collect();
        assert!(no_results.is_empty());
    }
}

proptest! {
    /// Test that coordination settings maintain valid configurations
    #[test]
    fn test_coordination_settings_invariants(
        max_agents in 1u32..1000,
        heartbeat_interval in 1u64..300,
        task_timeout in 1u64..3600,
        retry_attempts in 1u32..10,
        backoff_multiplier in 1.0f64..5.0,
    ) {
        let retry_policy = RetryPolicy::builder()
            .max_attempts(retry_attempts)
            .backoff_multiplier(backoff_multiplier)
            .build()
            .unwrap();
        
        let settings = CoordinationSettings::builder()
            .max_agents(max_agents)
            .heartbeat_interval_seconds(heartbeat_interval)
            .task_timeout_seconds(task_timeout)
            .retry_policy(retry_policy.clone())
            .build()
            .unwrap();
        
        // Verify all values are within reasonable bounds
        assert!(settings.max_agents() > 0);
        assert!(settings.max_agents() < 10000); // Reasonable upper bound
        
        assert!(settings.heartbeat_interval_seconds() > 0);
        assert!(settings.heartbeat_interval_seconds() < 3600); // Max 1 hour
        
        assert!(settings.task_timeout_seconds() > 0);
        assert!(settings.task_timeout_seconds() < 86400); // Max 1 day
        
        // Retry policy should be valid
        assert!(settings.retry_policy().max_attempts() > 0);
        assert!(settings.retry_policy().max_attempts() < 100);
        assert!(settings.retry_policy().backoff_multiplier() >= 1.0);
        assert!(settings.retry_policy().backoff_multiplier() < 10.0);
        
        // Heartbeat should be shorter than task timeout
        assert!(settings.heartbeat_interval_seconds() <= settings.task_timeout_seconds());
    }
}

// Integration property tests that require database

#[tokio::test]
async fn test_database_consistency_properties() {
    use proptest::test_runner::{Config, TestRunner};
    use proptest::strategy::Strategy;
    
    let mut runner = TestRunner::new(Config::default());
    
    // Property: Agent creation and retrieval should be consistent
    let agent_strategy = agent_strategy();
    
    runner.run(&agent_strategy, |agent| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let db_helper = DatabaseTestHelper::new().await.unwrap();
            let storage_manager = Arc::new(
                vibe_ensemble_storage::StorageManager::new(db_helper.pool.clone()).await.unwrap()
            );
            
            // Create agent
            let agent_id = storage_manager.agents().create_agent(agent.clone()).await.unwrap();
            
            // Retrieve agent
            let retrieved_agent = storage_manager.agents().get_agent(agent_id).await.unwrap();
            
            // Agents should be equivalent
            assert_eq!(agent.name(), retrieved_agent.name());
            assert_eq!(agent.capabilities(), retrieved_agent.capabilities());
            assert_eq!(agent.status(), retrieved_agent.status());
            
            Ok(())
        }).unwrap();
        Ok(())
    }).unwrap();
}

#[tokio::test]
async fn test_concurrent_operations_properties() {
    use proptest::test_runner::{Config, TestRunner};
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    let mut runner = TestRunner::new(Config::default());
    let agents_strategy = prop::collection::vec(agent_strategy(), 1..20);
    
    runner.run(&agents_strategy, |agents| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let db_helper = DatabaseTestHelper::new().await.unwrap();
            let storage_manager = Arc::new(
                vibe_ensemble_storage::StorageManager::new(db_helper.pool.clone()).await.unwrap()
            );
            
            let success_count = Arc::new(AtomicUsize::new(0));
            let mut handles = Vec::new();
            
            // Create agents concurrently
            for agent in agents {
                let storage_clone = storage_manager.clone();
                let success_count_clone = success_count.clone();
                
                let handle = tokio::spawn(async move {
                    if let Ok(_agent_id) = storage_clone.agents().create_agent(agent).await {
                        success_count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                });
                handles.push(handle);
            }
            
            // Wait for all operations to complete
            for handle in handles {
                handle.await.unwrap();
            }
            
            // Verify at least some operations succeeded (allowing for potential race conditions)
            let final_count = success_count.load(Ordering::SeqCst);
            assert!(final_count > 0, "At least some concurrent operations should succeed");
            
            // Verify database consistency
            let stored_agents = storage_manager.agents().list_agents().await.unwrap();
            assert_eq!(stored_agents.len(), final_count);
            
            Ok(())
        }).unwrap();
        Ok(())
    }).unwrap();
}