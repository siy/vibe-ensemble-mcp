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
        assert_eq!(init_result.server_info.version, "0.1.0");

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

        assert!(tool_names.contains(&"agent_register"));
        assert!(tool_names.contains(&"issue_create"));

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
}
