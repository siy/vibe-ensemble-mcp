//! Integration tests for MCP protocol compliance
//!
//! These tests verify the complete MCP protocol implementation including
//! initialization, capability negotiation, tool discovery, and error handling.

#[cfg(test)]
mod tests {
    use crate::{client::McpClient, protocol::*, server::McpServer, transport::TransportFactory};
    use serde_json::json;
    use tokio::time::{timeout, Duration};

    /// Test complete MCP initialization handshake
    #[tokio::test]
    async fn test_mcp_initialization_handshake() {
        let (client_transport, server_transport) = TransportFactory::in_memory_pair();

        // Create client
        let client_info = ClientInfo {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client_capabilities = ClientCapabilities {
            experimental: None,
            sampling: None,
        };

        let mut client = McpClient::new(
            client_transport,
            client_info.clone(),
            client_capabilities.clone(),
        );

        // Create server
        let server = McpServer::new();

        // Simulate server-side message handling
        let server_handle = {
            let server = server.clone();
            let mut transport = server_transport;

            tokio::spawn(async move {
                // Receive initialization request
                let request_json = transport.receive().await.unwrap();
                let response = server.handle_message(&request_json).await.unwrap();

                if let Some(response_json) = response {
                    transport.send(&response_json).await.unwrap();
                }
            })
        };

        // Client initialization
        let result = timeout(Duration::from_secs(5), client.initialize()).await;
        assert!(
            result.is_ok(),
            "Initialization should complete within timeout"
        );

        let init_result = result.unwrap().unwrap();
        assert_eq!(init_result.protocol_version, MCP_VERSION);
        assert_eq!(init_result.server_info.name, "vibe-ensemble-mcp");
        assert_eq!(init_result.server_info.version, env!("CARGO_PKG_VERSION"));

        // Wait for server task to complete
        server_handle.await.unwrap();
    }

    /// Test ping-pong for connection health checks
    #[tokio::test]
    async fn test_ping_pong() {
        let (client_transport, server_transport) = TransportFactory::in_memory_pair();

        let client_info = ClientInfo {
            name: "ping-test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client_capabilities = ClientCapabilities {
            experimental: None,
            sampling: None,
        };

        let mut client = McpClient::new(client_transport, client_info, client_capabilities);
        let server = McpServer::new();

        // Server task to handle ping
        let server_handle = {
            let server = server.clone();
            let mut transport = server_transport;

            tokio::spawn(async move {
                // Handle initialization first
                let init_request = transport.receive().await.unwrap();
                let init_response = server.handle_message(&init_request).await.unwrap().unwrap();
                transport.send(&init_response).await.unwrap();

                // Handle ping request
                let ping_request = transport.receive().await.unwrap();
                let ping_response = server.handle_message(&ping_request).await.unwrap().unwrap();
                transport.send(&ping_response).await.unwrap();
            })
        };

        // Initialize and then ping
        client.initialize().await.unwrap();
        let ping_result = client.ping().await.unwrap();

        assert!(ping_result.get("timestamp").is_some());
        assert_eq!(ping_result.get("server").unwrap(), "vibe-ensemble-mcp");

        server_handle.await.unwrap();
    }

    /// Test tool discovery
    #[tokio::test]
    async fn test_tool_discovery() {
        let (client_transport, server_transport) = TransportFactory::in_memory_pair();

        let client_info = ClientInfo {
            name: "tool-test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client_capabilities = ClientCapabilities {
            experimental: None,
            sampling: None,
        };

        let mut client = McpClient::new(client_transport, client_info, client_capabilities);
        let server = McpServer::new();

        // Server task
        let server_handle = {
            let server = server.clone();
            let mut transport = server_transport;

            tokio::spawn(async move {
                // Handle initialization
                let init_request = transport.receive().await.unwrap();
                let init_response = server.handle_message(&init_request).await.unwrap().unwrap();
                transport.send(&init_response).await.unwrap();

                // Handle list tools request
                let tools_request = transport.receive().await.unwrap();
                let tools_response = server
                    .handle_message(&tools_request)
                    .await
                    .unwrap()
                    .unwrap();
                transport.send(&tools_response).await.unwrap();
            })
        };

        // Initialize and list tools
        client.initialize().await.unwrap();
        let tools = client.list_tools().await.unwrap();

        let tools_array = tools.get("tools").unwrap().as_array().unwrap();
        assert!(!tools_array.is_empty());

        // Check for expected tools
        let tool_names: Vec<&str> = tools_array
            .iter()
            .map(|tool| tool.get("name").unwrap().as_str().unwrap())
            .collect();

        assert!(tool_names.contains(&"vibe_agent_register"));
        assert!(tool_names.contains(&"vibe_agent_status"));
        assert!(tool_names.contains(&"vibe_agent_list"));
        assert!(tool_names.contains(&"vibe_agent_deregister"));
        assert!(tool_names.contains(&"vibe_issue_create"));

        server_handle.await.unwrap();
    }

    /// Test agent registration (Vibe Ensemble extension)
    #[tokio::test]
    async fn test_agent_registration() {
        let (client_transport, server_transport) = TransportFactory::in_memory_pair();

        let client_info = ClientInfo {
            name: "agent-test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client_capabilities = ClientCapabilities {
            experimental: None,
            sampling: None,
        };

        let mut client = McpClient::new(client_transport, client_info, client_capabilities);
        let server = McpServer::new();

        // Server task
        let server_handle = {
            let server = server.clone();
            let mut transport = server_transport;

            tokio::spawn(async move {
                // Handle initialization
                let init_request = transport.receive().await.unwrap();
                let init_response = server.handle_message(&init_request).await.unwrap().unwrap();
                transport.send(&init_response).await.unwrap();

                // Handle agent registration
                let register_request = transport.receive().await.unwrap();
                let register_response = server
                    .handle_message(&register_request)
                    .await
                    .unwrap()
                    .unwrap();
                transport.send(&register_response).await.unwrap();
            })
        };

        // Initialize and register agent
        client.initialize().await.unwrap();

        let agent_params = AgentRegisterParams {
            name: "test-worker".to_string(),
            agent_type: "Worker".to_string(),
            capabilities: vec!["code-review".to_string(), "testing".to_string()],
            connection_metadata: json!({
                "endpoint": "ws://localhost:8080",
                "session": "test-session"
            }),
        };

        let registration_result = client.register_agent(agent_params).await.unwrap();

        assert_eq!(registration_result.status, "registered_fallback");
        assert!(!registration_result.assigned_resources.is_empty());

        server_handle.await.unwrap();
    }

    /// Test error handling for invalid requests
    #[tokio::test]
    async fn test_error_handling() {
        let (_client_transport, mut server_transport) = TransportFactory::in_memory_pair();

        let server = McpServer::new();

        // Send invalid JSON
        let invalid_json = "{invalid json}";
        server_transport.send(invalid_json).await.unwrap();

        let result = server.handle_message(invalid_json).await;
        assert!(result.is_err());

        // Send valid JSON but invalid method
        let invalid_request = JsonRpcRequest::new("invalid/method", None);
        let request_json = serde_json::to_string(&invalid_request).unwrap();

        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();

        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::METHOD_NOT_FOUND
        );
    }

    /// Test server capabilities
    #[tokio::test]
    async fn test_server_capabilities() {
        let server = McpServer::new();
        let capabilities = server.capabilities();

        // Check standard MCP capabilities
        assert!(capabilities.tools.is_some());
        assert!(capabilities.resources.is_some());
        assert!(capabilities.prompts.is_some());

        // Check Vibe Ensemble extensions
        assert_eq!(capabilities.vibe_agent_management, Some(true));
        assert_eq!(capabilities.vibe_issue_tracking, Some(true));
        assert_eq!(capabilities.vibe_messaging, Some(true));
        assert_eq!(capabilities.vibe_knowledge_management, Some(true));
    }

    /// Test connection lifecycle
    #[tokio::test]
    async fn test_connection_lifecycle() {
        let server = McpServer::new();

        // Initially no clients
        assert_eq!(server.client_count().await, 0);

        // Simulate client connection
        let init_request = JsonRpcRequest::new_with_id(
            serde_json::Value::String("test-session-1".to_string()),
            methods::INITIALIZE,
            Some(json!({
                "protocolVersion": MCP_VERSION,
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                },
                "capabilities": {}
            })),
        );

        let request_json = serde_json::to_string(&init_request).unwrap();
        let _response = server.handle_message(&request_json).await.unwrap();

        // Should have one client now
        assert_eq!(server.client_count().await, 1);
        assert!(server.is_client_connected("test-session-1").await);

        // Test disconnect
        assert!(server.disconnect_client("test-session-1").await);
        assert_eq!(server.client_count().await, 0);
        assert!(!server.is_client_connected("test-session-1").await);
    }

    /// Test agent status reporting functionality
    #[tokio::test]
    async fn test_agent_status_reporting() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{repositories::AgentRepository, services::AgentService};

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let server = McpServer::with_services(Some(agent_service), None, None, None, None);

        // Test status query (no parameters)
        let status_request = JsonRpcRequest::new(methods::AGENT_STATUS, None);
        let request_json = serde_json::to_string(&status_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert!(result.get("total_agents").is_some());
        assert!(result.get("online_agents").is_some());

        // Test status update with parameters
        let status_params = json!({
            "agentId": "550e8400-e29b-41d4-a716-446655440000",
            "status": "Online",
            "currentTask": "Processing data",
            "progress": 0.5
        });

        let status_update_request = JsonRpcRequest::new(methods::AGENT_STATUS, Some(status_params));
        let request_json = serde_json::to_string(&status_update_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(result.get("status").unwrap(), "acknowledged");
    }

    /// Test agent list functionality with various filters
    #[tokio::test]
    async fn test_agent_list_filtering() {
        use std::sync::Arc;
        use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};
        use vibe_ensemble_storage::{repositories::AgentRepository, services::AgentService};

        // Create in-memory database and add test agents
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo.clone()));
        let server = McpServer::with_services(Some(agent_service.clone()), None, None, None, None);

        // Create test agents
        let test_agent1 = Agent::new(
            "test-coordinator".to_string(),
            AgentType::Coordinator,
            vec!["coordination".to_string(), "planning".to_string()],
            ConnectionMetadata {
                endpoint: "ws://localhost:8080".to_string(),
                session_id: Some("session1".to_string()),
                protocol_version: "2024-11-05".to_string(),
            },
        )
        .unwrap();

        let test_agent2 = Agent::new(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["code-review".to_string(), "testing".to_string()],
            ConnectionMetadata {
                endpoint: "ws://localhost:8081".to_string(),
                session_id: Some("session2".to_string()),
                protocol_version: "2024-11-05".to_string(),
            },
        )
        .unwrap();

        agent_repo.create(&test_agent1).await.unwrap();
        agent_repo.create(&test_agent2).await.unwrap();

        // Test list all agents
        let list_request = JsonRpcRequest::new(methods::AGENT_LIST, None);
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        let agents = result.get("agents").unwrap().as_array().unwrap();
        assert_eq!(agents.len(), 2);

        // Test filter by agent type
        let filter_params = json!({
            "agentType": "Coordinator"
        });

        let filtered_request = JsonRpcRequest::new(methods::AGENT_LIST, Some(filter_params));
        let request_json = serde_json::to_string(&filtered_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let agents = result.get("agents").unwrap().as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].get("agent_type").unwrap(), "Coordinator");

        // Test filter by capability
        let capability_params = json!({
            "capability": "code-review"
        });

        let capability_request = JsonRpcRequest::new(methods::AGENT_LIST, Some(capability_params));
        let request_json = serde_json::to_string(&capability_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let agents = result.get("agents").unwrap().as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert!(agents[0]
            .get("capabilities")
            .unwrap()
            .as_array()
            .unwrap()
            .contains(&json!("code-review")));

        // Test limit parameter
        let limit_params = json!({
            "limit": 1
        });

        let limit_request = JsonRpcRequest::new(methods::AGENT_LIST, Some(limit_params));
        let request_json = serde_json::to_string(&limit_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let agents = result.get("agents").unwrap().as_array().unwrap();
        assert_eq!(agents.len(), 1);
    }

    /// Test agent deregistration functionality
    #[tokio::test]
    async fn test_agent_deregistration() {
        use std::sync::Arc;
        use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};
        use vibe_ensemble_storage::{repositories::AgentRepository, services::AgentService};

        // Create in-memory database and add test agent
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo.clone()));
        let server = McpServer::with_services(Some(agent_service.clone()), None, None, None, None);

        // Register an agent first
        let test_agent = Agent::new(
            "test-deregister".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            ConnectionMetadata {
                endpoint: "ws://localhost:8082".to_string(),
                session_id: Some("session3".to_string()),
                protocol_version: "2024-11-05".to_string(),
            },
        )
        .unwrap();

        agent_repo.create(&test_agent).await.unwrap();
        let agent_id = test_agent.id;

        // Test successful deregistration
        let deregister_params = json!({
            "agentId": agent_id.to_string(),
            "shutdownReason": "Test completion"
        });

        let deregister_request =
            JsonRpcRequest::new(methods::AGENT_DEREGISTER, Some(deregister_params));
        let request_json = serde_json::to_string(&deregister_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(result.get("status").unwrap(), "deregistered");
        assert_eq!(result.get("cleanupStatus").unwrap(), "completed");

        // Test deregistration of non-existent agent
        let invalid_params = json!({
            "agentId": "550e8400-e29b-41d4-a716-446655440000"
        });

        let invalid_request = JsonRpcRequest::new(methods::AGENT_DEREGISTER, Some(invalid_params));
        let request_json = serde_json::to_string(&invalid_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::AGENT_NOT_FOUND
        );
    }

    /// Test error handling for invalid agent management requests
    #[tokio::test]
    async fn test_agent_management_error_handling() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{repositories::AgentRepository, services::AgentService};

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let server = McpServer::with_services(Some(agent_service), None, None, None, None);

        // Test agent status with invalid agent ID
        let invalid_status_params = json!({
            "agentId": "invalid-uuid",
            "status": "Online"
        });

        let status_request =
            JsonRpcRequest::new(methods::AGENT_STATUS, Some(invalid_status_params));
        let request_json = serde_json::to_string(&status_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        if parsed_response.error.is_none() {
            println!(
                "Expected error but got result: {:?}",
                parsed_response.result
            );
        }
        assert!(parsed_response.error.is_some());

        // Test agent list with invalid agent type
        let invalid_list_params = json!({
            "agentType": "InvalidType"
        });

        let list_request = JsonRpcRequest::new(methods::AGENT_LIST, Some(invalid_list_params));
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());

        // Test agent deregister without required parameters
        let missing_params = json!({
            "shutdownReason": "Test"
        });

        let deregister_request =
            JsonRpcRequest::new(methods::AGENT_DEREGISTER, Some(missing_params));
        let request_json = serde_json::to_string(&deregister_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::INVALID_PARAMS
        );
    }

    /// Test comprehensive agent lifecycle through MCP tools
    #[tokio::test]
    async fn test_complete_agent_lifecycle() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{repositories::AgentRepository, services::AgentService};

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let server = McpServer::with_services(Some(agent_service), None, None, None, None);

        // 1. Register agent
        let register_params = json!({
            "name": "lifecycle-test-agent",
            "agentType": "Worker",
            "capabilities": ["code-review", "testing"],
            "connectionMetadata": {
                "endpoint": "ws://localhost:8080",
                "protocol_version": "2024-11-05",
                "session_id": "lifecycle-session"
            }
        });

        let register_request = JsonRpcRequest::new(methods::AGENT_REGISTER, Some(register_params));
        let request_json = serde_json::to_string(&register_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let register_result = parsed_response.result.unwrap();
        let agent_id = register_result.get("agentId").unwrap().as_str().unwrap();

        // 2. Report status
        let status_params = json!({
            "agentId": agent_id,
            "status": "Busy",
            "currentTask": "Running tests",
            "progress": 0.75
        });

        let status_request = JsonRpcRequest::new(methods::AGENT_STATUS, Some(status_params));
        let request_json = serde_json::to_string(&status_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        // 3. List agents and verify presence
        let list_request = JsonRpcRequest::new(methods::AGENT_LIST, None);
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let agents = result.get("agents").unwrap().as_array().unwrap();

        let found_agent = agents
            .iter()
            .find(|agent| agent.get("id").unwrap().as_str().unwrap() == agent_id);
        assert!(found_agent.is_some());

        // 4. Deregister agent
        let deregister_params = json!({
            "agentId": agent_id,
            "shutdownReason": "Lifecycle test complete"
        });

        let deregister_request =
            JsonRpcRequest::new(methods::AGENT_DEREGISTER, Some(deregister_params));
        let request_json = serde_json::to_string(&deregister_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let deregister_result = parsed_response.result.unwrap();
        assert_eq!(deregister_result.get("status").unwrap(), "deregistered");
    }

    /// Test comprehensive issue tracking MCP tools
    #[tokio::test]
    async fn test_issue_tracking_tools() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{
            repositories::{AgentRepository, IssueRepository},
            services::{AgentService, IssueService},
        };

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool.clone()));
        let issue_repo = Arc::new(IssueRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let issue_service = Arc::new(IssueService::new(issue_repo));

        let server = McpServer::with_services(Some(agent_service), Some(issue_service), None, None, None);

        // Create test agent for issue operations
        let register_params = json!({
            "name": "test-issue-agent",
            "agentType": "Worker",
            "capabilities": ["issue_tracking"],
            "connectionMetadata": {
                "endpoint": "test://localhost:8080",
                "protocol_version": "websocket-1.0"
            }
        });

        let register_request = JsonRpcRequest::new(methods::AGENT_REGISTER, Some(register_params));
        let request_json = serde_json::to_string(&register_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        if parsed_response.error.is_some() {
            println!("Agent registration error: {:?}", parsed_response.error);
        }
        assert!(parsed_response.result.is_some());
        let result = parsed_response.result.unwrap();
        let registered_agent_id = result.get("agentId").unwrap().as_str().unwrap();

        // Test 1: Create issue (vibe/issue/create)
        let create_params = json!({
            "title": "Cross-project dependency issue",
            "description": "Need to coordinate changes across multiple repositories",
            "priority": "High",
            "issueType": "coordination",
            "projectId": "vibe-ensemble-mcp",
            "createdByAgentId": registered_agent_id,
            "labels": ["coordination", "cross-project", "urgent"]
        });

        let create_request = JsonRpcRequest::new(methods::ISSUE_CREATE, Some(create_params));
        let request_json = serde_json::to_string(&create_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(
            result.get("title").unwrap(),
            "Cross-project dependency issue"
        );
        assert_eq!(result.get("priority").unwrap(), "High");
        assert_eq!(result.get("status").unwrap(), "Open");
        let created_issue_id = result.get("issueId").unwrap().as_str().unwrap();

        // Test 2: List issues (vibe/issue/list)
        let list_params = json!({
            "priority": "High",
            "limit": 10
        });

        let list_request = JsonRpcRequest::new(methods::ISSUE_LIST, Some(list_params));
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        let issues = result.get("issues").unwrap().as_array().unwrap();
        assert!(!issues.is_empty());
        assert_eq!(
            result.get("total").unwrap().as_u64().unwrap(),
            issues.len() as u64
        );

        let issue = &issues[0];
        assert_eq!(
            issue.get("title").unwrap(),
            "Cross-project dependency issue"
        );
        assert_eq!(issue.get("priority").unwrap(), "High");

        // Test 3: Assign issue (vibe/issue/assign)
        let assign_params = json!({
            "issueId": created_issue_id,
            "assigneeAgentId": registered_agent_id,
            "assignedByAgentId": registered_agent_id,
            "reason": "Agent has the required coordination capabilities"
        });

        let assign_request = JsonRpcRequest::new(methods::ISSUE_ASSIGN, Some(assign_params));
        let request_json = serde_json::to_string(&assign_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(
            result.get("issueId").unwrap().as_str().unwrap(),
            created_issue_id
        );
        assert_eq!(
            result.get("assigneeAgentId").unwrap().as_str().unwrap(),
            registered_agent_id
        );
        assert_eq!(
            result.get("message").unwrap(),
            "Issue assigned successfully"
        );

        // Test 4: Update issue status and priority (vibe/issue/update)
        let update_params = json!({
            "issueId": created_issue_id,
            "status": "Resolved",
            "updatedByAgentId": registered_agent_id,
            "priority": "Critical",
            "comment": "Work completed on coordinating the changes"
        });

        let update_request = JsonRpcRequest::new(methods::ISSUE_UPDATE, Some(update_params));
        let request_json = serde_json::to_string(&update_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        if parsed_response.error.is_some() {
            println!("Issue update error: {:?}", parsed_response.error);
        }
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(
            result.get("issueId").unwrap().as_str().unwrap(),
            created_issue_id
        );
        assert_eq!(result.get("status").unwrap(), "Resolved");
        assert_eq!(result.get("priority").unwrap(), &json!("Critical"));
        assert_eq!(result.get("commentAdded").unwrap(), true);

        // Test 5: List assigned issues
        let assigned_list_params = json!({
            "assignee": registered_agent_id,
            "status": "Resolved"
        });

        let assigned_list_request =
            JsonRpcRequest::new(methods::ISSUE_LIST, Some(assigned_list_params));
        let request_json = serde_json::to_string(&assigned_list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        let issues = result.get("issues").unwrap().as_array().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].get("status").unwrap(), "Resolved");
        assert_eq!(issues[0].get("isAssigned").unwrap(), true);

        // Test 6: Close issue (vibe/issue/close)
        let close_params = json!({
            "issueId": created_issue_id,
            "closedByAgentId": registered_agent_id,
            "resolution": "Successfully coordinated changes across all affected repositories",
            "closeReason": "Task completed successfully"
        });

        let close_request = JsonRpcRequest::new(methods::ISSUE_CLOSE, Some(close_params));
        let request_json = serde_json::to_string(&close_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());

        let result = parsed_response.result.unwrap();
        assert_eq!(
            result.get("issueId").unwrap().as_str().unwrap(),
            created_issue_id
        );
        assert_eq!(result.get("status").unwrap(), "Closed");
        assert_eq!(
            result.get("resolution").unwrap(),
            "Successfully coordinated changes across all affected repositories"
        );
    }

    /// Test issue tracking error handling
    #[tokio::test]
    async fn test_issue_tracking_error_handling() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{
            repositories::{AgentRepository, IssueRepository},
            services::{AgentService, IssueService},
        };

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool.clone()));
        let issue_repo = Arc::new(IssueRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let issue_service = Arc::new(IssueService::new(issue_repo));

        let server = McpServer::with_services(Some(agent_service), Some(issue_service), None, None, None);

        // Test create issue with missing required fields
        let invalid_create_params = json!({
            "title": "Test issue"
            // Missing description and createdByAgentId
        });

        let create_request =
            JsonRpcRequest::new(methods::ISSUE_CREATE, Some(invalid_create_params));
        let request_json = serde_json::to_string(&create_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::INVALID_PARAMS
        );

        // Test assign issue with invalid issue ID
        let invalid_assign_params = json!({
            "issueId": "invalid-uuid",
            "assigneeAgentId": "550e8400-e29b-41d4-a716-446655440000",
            "assignedByAgentId": "550e8400-e29b-41d4-a716-446655440001"
        });

        let assign_request =
            JsonRpcRequest::new(methods::ISSUE_ASSIGN, Some(invalid_assign_params));
        let request_json = serde_json::to_string(&assign_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::INVALID_PARAMS
        );

        // Test update issue with invalid status
        let invalid_update_params = json!({
            "issueId": "550e8400-e29b-41d4-a716-446655440000",
            "status": "InvalidStatus",
            "updatedByAgentId": "550e8400-e29b-41d4-a716-446655440001"
        });

        let update_request =
            JsonRpcRequest::new(methods::ISSUE_UPDATE, Some(invalid_update_params));
        let request_json = serde_json::to_string(&update_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::INVALID_PARAMS
        );

        // Test list issues with invalid priority filter
        let invalid_list_params = json!({
            "priority": "InvalidPriority"
        });

        let list_request = JsonRpcRequest::new(methods::ISSUE_LIST, Some(invalid_list_params));
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();

        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.error.is_some());
        assert_eq!(
            parsed_response.error.unwrap().code,
            error_codes::INVALID_PARAMS
        );
    }

    /// Test issue tracking workflow scenarios
    #[tokio::test]
    async fn test_issue_tracking_workflows() {
        use std::sync::Arc;
        use vibe_ensemble_storage::{
            repositories::{AgentRepository, IssueRepository},
            services::{AgentService, IssueService},
        };

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:?cache=shared")
            .await
            .unwrap();
        vibe_ensemble_storage::migrations::run_migrations(&pool)
            .await
            .unwrap();

        let agent_repo = Arc::new(AgentRepository::new(pool.clone()));
        let issue_repo = Arc::new(IssueRepository::new(pool));
        let agent_service = Arc::new(AgentService::new(agent_repo));
        let issue_service = Arc::new(IssueService::new(issue_repo));

        let server = McpServer::with_services(Some(agent_service), Some(issue_service), None, None, None);

        // Register coordinator agent
        let coordinator_params = json!({
            "name": "coordinator-agent",
            "agentType": "Coordinator",
            "capabilities": ["coordination", "issue_management"],
            "connectionMetadata": {
                "endpoint": "test://coordinator:8080",
                "protocol_version": "websocket-1.0"
            }
        });

        let register_request =
            JsonRpcRequest::new(methods::AGENT_REGISTER, Some(coordinator_params));
        let request_json = serde_json::to_string(&register_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let coordinator_id = parsed_response
            .result
            .unwrap()
            .get("agentId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Register worker agent
        let worker_params = json!({
            "name": "worker-agent",
            "agentType": "Worker",
            "capabilities": ["rust", "testing", "debugging"],
            "connectionMetadata": {
                "endpoint": "test://worker:8080",
                "protocol_version": "websocket-1.0"
            }
        });

        let register_request = JsonRpcRequest::new(methods::AGENT_REGISTER, Some(worker_params));
        let request_json = serde_json::to_string(&register_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let worker_id = parsed_response
            .result
            .unwrap()
            .get("agentId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Workflow 1: Coordinator creates coordination issue
        let create_params = json!({
            "title": "Implement distributed testing coordination",
            "description": "Need to coordinate testing across multiple worker agents to ensure consistency",
            "priority": "Medium",
            "issueType": "coordination",
            "projectId": "testing-infrastructure",
            "createdByAgentId": coordinator_id,
            "labels": ["testing", "coordination", "infrastructure"],
            "assignee": worker_id
        });

        let create_request = JsonRpcRequest::new(methods::ISSUE_CREATE, Some(create_params));
        let request_json = serde_json::to_string(&create_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let issue_id = parsed_response
            .result
            .unwrap()
            .get("issueId")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Worker updates issue status to indicate work progressed
        let update_params = json!({
            "issueId": issue_id,
            "status": "Resolved",
            "updatedByAgentId": worker_id,
            "comment": "Completed initial work on testing coordination implementation"
        });

        let update_request = JsonRpcRequest::new(methods::ISSUE_UPDATE, Some(update_params));
        let request_json = serde_json::to_string(&update_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        if parsed_response.error.is_some() {
            println!("Workflow update error: {:?}", parsed_response.error);
        }
        assert!(parsed_response.result.is_some());

        // Coordinator queries issues to monitor progress
        let list_params = json!({
            "assignee": worker_id,
            "status": "Resolved"
        });

        let list_request = JsonRpcRequest::new(methods::ISSUE_LIST, Some(list_params));
        let request_json = serde_json::to_string(&list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let issues = result.get("issues").unwrap().as_array().unwrap();
        assert_eq!(issues.len(), 1);

        // Worker completes work and closes issue
        let close_params = json!({
            "issueId": issue_id,
            "closedByAgentId": worker_id,
            "resolution": "Implemented distributed testing coordination with proper synchronization",
            "closeReason": "Feature implemented and tested successfully"
        });

        let close_request = JsonRpcRequest::new(methods::ISSUE_CLOSE, Some(close_params));
        let request_json = serde_json::to_string(&close_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(parsed_response.result.is_some());
        assert_eq!(
            parsed_response.result.unwrap().get("status").unwrap(),
            "Closed"
        );

        // Final verification: List all closed issues
        let closed_list_params = json!({
            "status": "Closed"
        });

        let closed_list_request =
            JsonRpcRequest::new(methods::ISSUE_LIST, Some(closed_list_params));
        let request_json = serde_json::to_string(&closed_list_request).unwrap();
        let response = server.handle_message(&request_json).await.unwrap().unwrap();
        let parsed_response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        let result = parsed_response.result.unwrap();
        let issues = result.get("issues").unwrap().as_array().unwrap();
        assert!(!issues.is_empty());
        assert_eq!(
            issues
                .iter()
                .find(|issue| issue.get("id").unwrap().as_str().unwrap() == issue_id)
                .unwrap()
                .get("isTerminal")
                .unwrap(),
            true
        );
    }
}
