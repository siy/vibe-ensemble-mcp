//! Tests for worker communication MCP tools
//!
//! This module provides comprehensive tests for the four communication MCP tools:
//! - vibe/worker/message - Direct worker messaging
//! - vibe/worker/request - Action requests between workers
//! - vibe/worker/coordinate - Multi-worker coordination
//! - vibe/project/lock - Resource locking

#![allow(deprecated)] // Allow deprecated method constants for backward compatibility

#[cfg(test)]
mod tests {
    use crate::protocol::*;
    use crate::server::CoordinationServices;
    use crate::server::McpServer;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;
    use vibe_ensemble_core::agent::{AgentType, ConnectionMetadata};
    use vibe_ensemble_core::orchestration::WorkspaceManager;
    use vibe_ensemble_storage::repositories::{
        AgentRepository, IssueRepository, KnowledgeRepository, MessageRepository, ProjectRepository,
    };
    use vibe_ensemble_storage::services::{
        AgentService, CoordinationService, IssueService, KnowledgeService, MessageService,
    };

    async fn setup_test_server() -> (McpServer, Arc<AgentService>, Arc<MessageService>) {
        // Create in-memory database
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        // Create repositories
        let agent_repo = Arc::new(AgentRepository::new(pool.clone()));
        let issue_repo = Arc::new(IssueRepository::new(pool.clone()));
        let message_repo = Arc::new(MessageRepository::new(pool.clone()));
        let knowledge_repo = Arc::new(KnowledgeRepository::new(pool.clone()));
        let project_repo = Arc::new(ProjectRepository::new(pool));

        // Create services
        let agent_service = Arc::new(AgentService::new(agent_repo.clone()));
        let issue_service = Arc::new(IssueService::new(issue_repo.clone()));
        let message_service = Arc::new(MessageService::new(message_repo.clone()));
        let knowledge_service = Arc::new(KnowledgeService::new((*knowledge_repo).clone()));
        let coordination_service = Arc::new(CoordinationService::new(
            agent_repo,
            issue_repo,
            message_repo,
            project_repo,
        ));

        // Create workspace manager for tests
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let workspace_manager = Arc::new(WorkspaceManager::new(temp_dir.path()));

        let coordination_services = CoordinationServices::new(
            agent_service.clone(),
            issue_service,
            message_service.clone(),
            coordination_service,
            knowledge_service,
            workspace_manager,
        );
        let server = McpServer::with_coordination(coordination_services);

        (server, agent_service, message_service)
    }

    async fn create_test_agents(agent_service: &AgentService) -> (Uuid, Uuid, Uuid) {
        // Create coordinator agent
        let coordinator = agent_service
            .register_agent(
                "test-coordinator".to_string(),
                AgentType::Coordinator,
                vec!["coordination".to_string(), "management".to_string()],
                ConnectionMetadata::new(
                    "http://localhost:8080".to_string(),
                    "test-protocol-1.0".to_string(),
                    Some("coord-session".to_string()),
                )
                .unwrap(),
                "coord-session".to_string(),
            )
            .await
            .unwrap();

        // Create worker agents
        let worker1 = agent_service
            .register_agent(
                "test-worker-1".to_string(),
                AgentType::Worker,
                vec!["rust".to_string(), "testing".to_string()],
                ConnectionMetadata::new(
                    "http://localhost:8080".to_string(),
                    "test-protocol-1.0".to_string(),
                    Some("worker1-session".to_string()),
                )
                .unwrap(),
                "worker1-session".to_string(),
            )
            .await
            .unwrap();

        let worker2 = agent_service
            .register_agent(
                "test-worker-2".to_string(),
                AgentType::Worker,
                vec!["frontend".to_string(), "react".to_string()],
                ConnectionMetadata::new(
                    "http://localhost:8080".to_string(),
                    "test-protocol-1.0".to_string(),
                    Some("worker2-session".to_string()),
                )
                .unwrap(),
                "worker2-session".to_string(),
            )
            .await
            .unwrap();

        (coordinator.id, worker1.id, worker2.id)
    }

    #[tokio::test]
    async fn test_worker_message_success() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, worker2_id) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_message",
                "params": {
                    "recipientAgentId": worker2_id.to_string(),
                    "messageContent": "Hey, can you help me with the API integration?",
                    "messageType": "Request",
                    "senderAgentId": worker1_id.to_string(),
                    "priority": "High",
                    "metadata": {"project": "test-project"}
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result: WorkerMessageResult =
            serde_json::from_value(response_obj.result.unwrap()).unwrap();
        assert_eq!(result.sender_agent_id, worker1_id);
        assert_eq!(result.recipient_agent_id, worker2_id);
        assert_eq!(result.status, "sent");
        assert_eq!(result.message, "Worker message sent successfully");
    }

    #[tokio::test]
    async fn test_worker_message_invalid_agent() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;
        let nonexistent_agent_id = Uuid::new_v4();

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_message",
                "params": {
                    "recipientAgentId": nonexistent_agent_id.to_string(),
                    "messageContent": "This should fail",
                    "messageType": "Info",
                    "senderAgentId": worker1_id.to_string(),
                    "priority": "Normal"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::AGENT_NOT_FOUND);
        assert!(error.message.contains("Recipient agent not found"));
    }

    #[tokio::test]
    async fn test_worker_message_invalid_message_type() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, worker2_id) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_message",
                "params": {
                    "recipientAgentId": worker2_id.to_string(),
                    "messageContent": "Test message",
                    "messageType": "InvalidType",
                    "senderAgentId": worker1_id.to_string(),
                    "priority": "Normal"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
        assert!(error.message.contains("Invalid message type"));
    }

    #[tokio::test]
    async fn test_worker_request_success() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, worker2_id) = create_test_agents(&agent_service).await;
        let deadline = Utc::now() + chrono::Duration::hours(2);

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_request",
                "params": {
                    "targetAgentId": worker2_id.to_string(),
                    "requestType": "update_api_interface",
                    "requestDetails": {
                        "endpoint": "/api/users",
                        "changes": ["add_pagination", "update_schema"]
                    },
                    "requestedByAgentId": worker1_id.to_string(),
                    "deadline": deadline.to_rfc3339(),
                    "priority": "Urgent"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result: WorkerRequestResult =
            serde_json::from_value(response_obj.result.unwrap()).unwrap();
        assert_eq!(result.target_agent_id, worker2_id);
        assert_eq!(result.requested_by_agent_id, worker1_id);
        assert_eq!(result.request_type, "update_api_interface");
        assert_eq!(result.status, "sent");
        assert!(result.deadline.is_some());
        assert_eq!(result.message, "Worker request sent successfully");
    }

    #[tokio::test]
    async fn test_worker_request_missing_target() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_request",
                "params": {
                    "requestType": "test_request",
                    "requestDetails": {"key": "value"},
                    "requestedByAgentId": worker1_id.to_string()
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_worker_coordinate_success() {
        let (server, agent_service, _) = setup_test_server().await;
        let (coordinator_id, worker1_id, worker2_id) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_coordinate",
                "params": {
                    "coordinationType": "merge_preparation",
                    "involvedAgents": [worker1_id.to_string(), worker2_id.to_string()],
                    "scope": {
                        "files": ["src/api/users.rs", "frontend/components/UserList.tsx"],
                        "modules": ["user_management"],
                        "project": "main"
                    },
                    "coordinatorAgentId": coordinator_id.to_string(),
                    "details": {
                        "reason": "Upcoming merge conflict resolution",
                        "timeline": "next_2_hours",
                        "requirements": ["backup_changes", "sync_branches"]
                    }
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result: WorkerCoordinateResult =
            serde_json::from_value(response_obj.result.unwrap()).unwrap();
        assert_eq!(result.coordinator_agent_id, coordinator_id);
        assert_eq!(result.involved_agents.len(), 2);
        assert!(result.involved_agents.contains(&worker1_id));
        assert!(result.involved_agents.contains(&worker2_id));
        assert_eq!(result.coordination_type, "merge_preparation");
        assert_eq!(result.status, "initiated");
        assert_eq!(result.participant_confirmations.len(), 2);
        assert_eq!(
            result.message,
            "Coordination session initiated successfully"
        );
    }

    #[tokio::test]
    async fn test_worker_coordinate_empty_agents() {
        let (server, agent_service, _) = setup_test_server().await;
        let (coordinator_id, _, _) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "worker_coordinate",
                "params": {
                    "coordinationType": "test_coordination",
                    "involvedAgents": [],
                    "scope": {"project": "test"},
                    "coordinatorAgentId": coordinator_id.to_string(),
                    "details": {"reason": "test"}
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
        assert!(error
            .message
            .contains("At least one involved agent is required"));
    }

    #[tokio::test]
    async fn test_project_lock_success() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "project_lock",
                "params": {
                    "projectId": "test-project",
                    "resourcePath": "src/database/migrations",
                    "lockType": "Exclusive",
                    "lockHolderAgentId": worker1_id.to_string(),
                    "duration": 3600, // 1 hour
                    "reason": "Database schema migration in progress"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result: ProjectLockResult =
            serde_json::from_value(response_obj.result.unwrap()).unwrap();
        assert_eq!(result.project_id, Some("test-project".to_string()));
        assert_eq!(result.resource_path, "src/database/migrations");
        assert_eq!(result.lock_type, "Exclusive");
        assert_eq!(result.lock_holder_agent_id, worker1_id);
        assert_eq!(result.status, "acquired");
        assert!(result.expiration.is_some());
        assert_eq!(result.message, "Project lock acquired successfully");
    }

    #[tokio::test]
    async fn test_project_lock_invalid_type() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "project_lock",
                "params": {
                    "resourcePath": "src/test.rs",
                    "lockType": "InvalidLockType",
                    "lockHolderAgentId": worker1_id.to_string(),
                    "reason": "Testing invalid lock type"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
        assert!(error.message.contains("Invalid lock type"));
    }

    #[tokio::test]
    async fn test_project_lock_without_duration() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "project_lock",
                "params": {
                    "resourcePath": "config/settings.toml",
                    "lockType": "Shared",
                    "lockHolderAgentId": worker1_id.to_string(),
                    "reason": "Reading configuration"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result: ProjectLockResult =
            serde_json::from_value(response_obj.result.unwrap()).unwrap();
        assert_eq!(result.lock_type, "Shared");
        assert!(result.expiration.is_none()); // No duration specified
        assert_eq!(result.status, "acquired");
    }

    #[tokio::test]
    async fn test_project_lock_nonexistent_agent() {
        let (server, agent_service, _) = setup_test_server().await;
        let _ = create_test_agents(&agent_service).await;
        let nonexistent_agent_id = Uuid::new_v4();

        let request = JsonRpcRequest::new(
            "vibe/coordination",
            Some(json!({
                "operation": "project_lock",
                "params": {
                    "resourcePath": "src/main.rs",
                    "lockType": "Exclusive",
                    "lockHolderAgentId": nonexistent_agent_id.to_string(),
                    "reason": "Testing with nonexistent agent"
                }
            })),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_some());

        let error = response_obj.error.unwrap();
        assert_eq!(error.code, error_codes::AGENT_NOT_FOUND);
        assert!(error.message.contains("Lock holder agent not found"));
    }

    #[tokio::test]
    async fn test_communication_tools_in_tools_list() {
        let (server, _, _) = setup_test_server().await;

        let request = JsonRpcRequest::new(methods::LIST_TOOLS, None);

        let response = server
            .handle_message(&serde_json::to_string(&request).unwrap())
            .await
            .unwrap()
            .unwrap();

        let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response_obj.error.is_none());
        assert!(response_obj.result.is_some());

        let result = response_obj.result.unwrap();
        let tools = result["tools"].as_array().unwrap();

        // Check that all four communication tools are listed
        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();

        assert!(tool_names.contains(&"vibe_worker_message"));
        assert!(tool_names.contains(&"vibe_worker_request"));
        assert!(tool_names.contains(&"vibe_worker_coordinate"));
        assert!(tool_names.contains(&"vibe_project_lock"));
    }

    #[tokio::test]
    async fn test_message_types_and_priorities() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, worker2_id) = create_test_agents(&agent_service).await;

        // Test all message types
        let message_types = ["Info", "Request", "Coordination", "Alert"];
        for msg_type in &message_types {
            let request = JsonRpcRequest::new(
                "vibe/coordination",
                Some(json!({
                    "operation": "worker_message",
                    "params": {
                        "recipientAgentId": worker2_id.to_string(),
                        "messageContent": format!("Test {} message", msg_type),
                        "messageType": msg_type,
                        "senderAgentId": worker1_id.to_string(),
                        "priority": "Normal"
                    }
                })),
            );

            let response = server
                .handle_message(&serde_json::to_string(&request).unwrap())
                .await
                .unwrap()
                .unwrap();

            let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
            assert!(
                response_obj.error.is_none(),
                "Failed for message type: {}",
                msg_type
            );
        }

        // Test all priorities
        let priorities = ["Low", "Normal", "High", "Urgent"];
        for priority in &priorities {
            let request = JsonRpcRequest::new(
                "vibe/coordination",
                Some(json!({
                    "operation": "worker_message",
                    "params": {
                        "recipientAgentId": worker2_id.to_string(),
                        "messageContent": format!("Test {} priority message", priority),
                        "messageType": "Info",
                        "senderAgentId": worker1_id.to_string(),
                        "priority": priority
                    }
                })),
            );

            let response = server
                .handle_message(&serde_json::to_string(&request).unwrap())
                .await
                .unwrap()
                .unwrap();

            let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
            assert!(
                response_obj.error.is_none(),
                "Failed for priority: {}",
                priority
            );
        }
    }

    #[tokio::test]
    async fn test_lock_types() {
        let (server, agent_service, _) = setup_test_server().await;
        let (_, worker1_id, _) = create_test_agents(&agent_service).await;

        // Test all lock types
        let lock_types = ["Exclusive", "Shared", "Coordination"];
        for lock_type in &lock_types {
            let request = JsonRpcRequest::new(
                "vibe/coordination",
                Some(json!({
                    "operation": "project_lock",
                    "params": {
                        "resourcePath": format!("test/{}.rs", lock_type.to_lowercase()),
                        "lockType": lock_type,
                        "lockHolderAgentId": worker1_id.to_string(),
                        "reason": format!("Testing {} lock", lock_type)
                    }
                })),
            );

            let response = server
                .handle_message(&serde_json::to_string(&request).unwrap())
                .await
                .unwrap()
                .unwrap();

            let response_obj: JsonRpcResponse = serde_json::from_str(&response).unwrap();
            assert!(
                response_obj.error.is_none(),
                "Failed for lock type: {}",
                lock_type
            );

            let result: ProjectLockResult =
                serde_json::from_value(response_obj.result.unwrap()).unwrap();
            assert_eq!(result.lock_type, *lock_type);
        }
    }
}
