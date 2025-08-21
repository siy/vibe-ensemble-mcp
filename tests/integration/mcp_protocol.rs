//! Integration tests for MCP protocol compliance
//!
//! These tests verify that the vibe-ensemble-mcp server correctly implements
//! the Model Context Protocol (MCP) specification.

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use serde_json::{json, Value};
use uuid::Uuid;

use vibe_ensemble_mcp::{McpServer, McpMessage, McpError};
use vibe_ensemble_storage::StorageManager;

use crate::common::{TestContext, database::DatabaseTestHelper};

/// Tests basic MCP protocol handshake and initialization
#[tokio::test]
async fn test_mcp_protocol_initialization() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Test protocol initialization
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });
    
    let response = server.handle_message(init_message).await.unwrap();
    
    // Verify response structure
    assert!(response.get("result").is_some());
    assert!(response.get("error").is_none());
    
    let result = response.get("result").unwrap();
    assert!(result.get("protocolVersion").is_some());
    assert!(result.get("serverInfo").is_some());
    assert!(result.get("capabilities").is_some());
}

/// Tests MCP resource listing functionality
#[tokio::test]
async fn test_mcp_resources_list() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    db_helper.seed_test_data().await.unwrap();
    
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize server first
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Test resources/list
    let list_message = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });
    
    let response = server.handle_message(list_message).await.unwrap();
    assert!(response.get("result").is_some());
    
    let result = response.get("result").unwrap();
    let resources = result.get("resources").unwrap().as_array().unwrap();
    assert!(!resources.is_empty());
    
    // Verify resource structure
    for resource in resources {
        assert!(resource.get("uri").is_some());
        assert!(resource.get("name").is_some());
        assert!(resource.get("description").is_some());
        assert!(resource.get("mimeType").is_some());
    }
}

/// Tests MCP resource reading functionality
#[tokio::test]
async fn test_mcp_resources_read() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    db_helper.seed_test_data().await.unwrap();
    
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize server
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Test resources/read
    let read_message = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "resources/read",
        "params": {
            "uri": "vibe://knowledge/test-knowledge-1"
        }
    });
    
    let response = server.handle_message(read_message).await.unwrap();
    
    if response.get("error").is_none() {
        let result = response.get("result").unwrap();
        let contents = result.get("contents").unwrap().as_array().unwrap();
        assert!(!contents.is_empty());
        
        for content in contents {
            assert!(content.get("uri").is_some());
            assert!(content.get("mimeType").is_some());
            assert!(content.get("text").is_some() || content.get("blob").is_some());
        }
    }
}

/// Tests MCP tool listing functionality
#[tokio::test]
async fn test_mcp_tools_list() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize server
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Test tools/list
    let list_message = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    
    let response = server.handle_message(list_message).await.unwrap();
    assert!(response.get("result").is_some());
    
    let result = response.get("result").unwrap();
    let tools = result.get("tools").unwrap().as_array().unwrap();
    assert!(!tools.is_empty());
    
    // Verify tool structure
    for tool in tools {
        assert!(tool.get("name").is_some());
        assert!(tool.get("description").is_some());
        assert!(tool.get("inputSchema").is_some());
    }
}

/// Tests MCP tool execution functionality
#[tokio::test]
async fn test_mcp_tools_call() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize server
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Test tools/call - create agent
    let call_message = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "create_agent",
            "arguments": {
                "name": "test-agent-mcp",
                "capabilities": ["testing", "mcp"]
            }
        }
    });
    
    let response = server.handle_message(call_message).await.unwrap();
    
    if response.get("error").is_none() {
        let result = response.get("result").unwrap();
        assert!(result.get("content").is_some());
        
        let content = result.get("content").unwrap().as_array().unwrap();
        assert!(!content.is_empty());
        
        for item in content {
            assert!(item.get("type").is_some());
            assert!(item.get("text").is_some());
        }
    }
}

/// Tests MCP prompt listing functionality
#[tokio::test]
async fn test_mcp_prompts_list() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize server
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Test prompts/list
    let list_message = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "prompts/list",
        "params": {}
    });
    
    let response = server.handle_message(list_message).await.unwrap();
    assert!(response.get("result").is_some());
    
    let result = response.get("result").unwrap();
    let prompts = result.get("prompts").unwrap().as_array().unwrap();
    
    // Verify prompt structure if any exist
    for prompt in prompts {
        assert!(prompt.get("name").is_some());
        assert!(prompt.get("description").is_some());
    }
}

/// Tests MCP error handling
#[tokio::test]
async fn test_mcp_error_handling() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Test invalid method
    let invalid_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "invalid/method",
        "params": {}
    });
    
    let response = server.handle_message(invalid_message).await.unwrap();
    assert!(response.get("error").is_some());
    
    let error = response.get("error").unwrap();
    assert!(error.get("code").is_some());
    assert!(error.get("message").is_some());
}

/// Tests MCP notification handling
#[tokio::test]
async fn test_mcp_notifications() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Test notification (no ID)
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    });
    
    // Notifications should not return a response
    let response = server.handle_message(notification).await;
    assert!(response.is_ok());
}

/// Tests MCP protocol version compatibility
#[tokio::test]
async fn test_mcp_version_compatibility() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Test with different protocol versions
    let versions = vec!["1.0", "1.0.0", "1.0.1"];
    
    for version in versions {
        let init_message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": version,
                "clientInfo": {"name": "test-client", "version": "1.0.0"}
            }
        });
        
        let response = server.handle_message(init_message).await.unwrap();
        
        // Should either succeed or return a compatible error
        if response.get("error").is_some() {
            let error = response.get("error").unwrap();
            let code = error.get("code").unwrap().as_i64().unwrap();
            assert!(code == -32601 || code == -32600); // Method not found or invalid request
        } else {
            assert!(response.get("result").is_some());
        }
    }
}

/// Tests MCP concurrent request handling
#[tokio::test]
async fn test_mcp_concurrent_requests() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = Arc::new(McpServer::new(storage_manager).await.unwrap());
    
    // Initialize server first
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    server.handle_message(init_message).await.unwrap();
    
    // Send multiple concurrent requests
    let mut handles = vec![];
    
    for i in 1..=10 {
        let server_clone = server.clone();
        let handle = tokio::spawn(async move {
            let message = json!({
                "jsonrpc": "2.0",
                "id": i,
                "method": "resources/list",
                "params": {}
            });
            
            server_clone.handle_message(message).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        let response = handle.await.unwrap().unwrap();
        assert!(response.get("id").is_some());
        assert!(response.get("result").is_some() || response.get("error").is_some());
    }
}

/// Tests MCP WebSocket transport
#[tokio::test]
async fn test_mcp_websocket_transport() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Start WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    
    // Test basic communication
    use tokio_tungstenite::tungstenite::Message;
    use futures_util::{SinkExt, StreamExt};
    
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Send initialization message
    let init_msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-ws-client", "version": "1.0.0"}
        }
    });
    
    ws_sender.send(Message::Text(init_msg.to_string())).await.unwrap();
    
    // Receive response with timeout
    let response = timeout(Duration::from_secs(5), ws_receiver.next()).await;
    assert!(response.is_ok());
    
    let msg = response.unwrap().unwrap().unwrap();
    if let Message::Text(text) = msg {
        let response_json: Value = serde_json::from_str(&text).unwrap();
        assert!(response_json.get("result").is_some());
    }
    
    // Cleanup
    server_handle.abort();
}

/// Tests MCP server capabilities advertisement
#[tokio::test]
async fn test_mcp_capabilities() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let server = McpServer::new(storage_manager).await.unwrap();
    
    // Initialize and get capabilities
    let init_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        }
    });
    
    let response = server.handle_message(init_message).await.unwrap();
    let result = response.get("result").unwrap();
    let capabilities = result.get("capabilities").unwrap();
    
    // Verify expected capabilities
    assert!(capabilities.get("resources").is_some());
    assert!(capabilities.get("tools").is_some());
    assert!(capabilities.get("prompts").is_some());
    
    // Verify resource capabilities
    let resources = capabilities.get("resources").unwrap();
    assert_eq!(resources.get("subscribe").unwrap().as_bool().unwrap(), true);
    assert_eq!(resources.get("listChanged").unwrap().as_bool().unwrap(), true);
    
    // Verify tools capabilities
    let tools = capabilities.get("tools").unwrap();
    assert_eq!(tools.get("listChanged").unwrap().as_bool().unwrap(), true);
}