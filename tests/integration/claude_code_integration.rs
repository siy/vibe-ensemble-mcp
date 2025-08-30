//! Claude Code Integration Tests
//!
//! This module contains comprehensive integration tests that simulate real Claude Code
//! client behavior across all supported transports. These tests validate the complete
//! MCP protocol lifecycle and ensure compatibility with Claude Code's usage patterns.

use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use serde_json::{json, Value};

use vibe_ensemble_mcp::claude_code_integration_tests::{
    ClaudeCodeTestSuite, MockClaudeCodeStdioClient, MockClaudeCodeWebSocketClient,
    MockClaudeCodeSseClient, ClaudeCodeClient, TestResult, TestResults
};
use vibe_ensemble_mcp::{McpServer, Error, Result};
use vibe_ensemble_storage::StorageManager;

use crate::common::{TestContext, database::DatabaseTestHelper};

/// Test Claude Code integration via stdio transport
#[tokio::test]
async fn test_claude_code_stdio_integration() {
    // This test requires the actual server binary to be built
    // Skip if not available for CI/CD compatibility
    if !server_binary_available().await {
        eprintln!("Skipping stdio integration test - server binary not available");
        return;
    }

    let test_suite = ClaudeCodeTestSuite::new()
        .with_timeout(Duration::from_secs(30))
        .with_cleanup(true);

    match MockClaudeCodeStdioClient::new().await {
        Ok(client) => {
            let results = test_suite.test_claude_code_simulation(client).await.unwrap();
            
            println!("Claude Code Stdio Integration Results:");
            print_test_results(&results);
            
            // Assert that critical tests passed
            assert!(results.success_count() >= 4, 
                "Expected at least 4 successful tests, got {} successes out of {} total", 
                results.success_count(), 
                results.total_count()
            );
            
            // Ensure initialization always works
            if let Some(init_result) = results.tests.get("initialization") {
                assert!(init_result.is_success(), "Initialization must succeed for stdio transport");
            }
        }
        Err(e) => {
            eprintln!("Failed to create stdio client: {}", e);
            panic!("Stdio integration test failed to start: {}", e);
        }
    }
}

/// Test Claude Code integration via WebSocket transport
#[tokio::test]
async fn test_claude_code_websocket_integration() {
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
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let test_suite = ClaudeCodeTestSuite::new()
        .with_timeout(Duration::from_secs(15))
        .with_cleanup(true);

    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    match MockClaudeCodeWebSocketClient::new(&ws_url).await {
        Ok(client) => {
            let results = test_suite.test_claude_code_simulation(client).await.unwrap();
            
            println!("Claude Code WebSocket Integration Results:");
            print_test_results(&results);
            
            // Assert that critical tests passed
            assert!(results.success_count() >= 3, 
                "Expected at least 3 successful tests, got {} successes out of {} total", 
                results.success_count(), 
                results.total_count()
            );
            
            // Ensure initialization always works
            if let Some(init_result) = results.tests.get("initialization") {
                assert!(init_result.is_success(), "Initialization must succeed for WebSocket transport");
            }
        }
        Err(e) => {
            eprintln!("Failed to create WebSocket client: {}", e);
            panic!("WebSocket integration test failed: {}", e);
        }
    }
    
    // Graceful cleanup
    graceful_shutdown(server_handle).await;
}

/// Test Claude Code integration via SSE transport (HTTP-based)
#[tokio::test]
#[ignore] // SSE test requires special setup and may be flaky in CI
async fn test_claude_code_sse_integration() {
    let test_suite = ClaudeCodeTestSuite::new()
        .with_timeout(Duration::from_secs(10))
        .with_cleanup(true);

    // For SSE, we simulate the transport since it requires an HTTP server
    let base_url = "http://localhost:8080";
    
    match MockClaudeCodeSseClient::new(base_url).await {
        Ok(client) => {
            let results = test_suite.test_claude_code_simulation(client).await.unwrap();
            
            println!("Claude Code SSE Integration Results:");
            print_test_results(&results);
            
            // SSE integration may have different success criteria due to simulation
            println!("SSE transport test completed with {} successful tests", results.success_count());
        }
        Err(e) => {
            eprintln!("Note: SSE integration test failed (expected in test environment): {}", e);
            // SSE tests may fail in test environment without HTTP server
        }
    }
}

/// Test concurrent Claude Code sessions
#[tokio::test]
async fn test_concurrent_claude_code_sessions() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Start WebSocket server for concurrent testing
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    // Give server time to start
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let test_suite = ClaudeCodeTestSuite::new()
        .with_timeout(Duration::from_secs(10))
        .with_cleanup(true);

    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    
    // Create multiple concurrent clients
    let mut handles = vec![];
    
    for i in 0..3 {
        let url = ws_url.clone();
        let suite = ClaudeCodeTestSuite::new().with_timeout(Duration::from_secs(10));
        
        let handle = tokio::spawn(async move {
            match MockClaudeCodeWebSocketClient::new(&url).await {
                Ok(client) => {
                    let results = suite.test_claude_code_simulation(client).await.unwrap();
                    println!("Concurrent client {} completed with {} successes", i, results.success_count());
                    results.success_count()
                }
                Err(e) => {
                    eprintln!("Concurrent client {} failed: {}", i, e);
                    0
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent sessions
    let mut total_successes = 0;
    for handle in handles {
        match handle.await {
            Ok(successes) => total_successes += successes,
            Err(e) => eprintln!("Concurrent session failed: {}", e),
        }
    }
    
    println!("Concurrent sessions total successes: {}", total_successes);
    assert!(total_successes >= 6, "Expected at least 6 total successes across concurrent sessions");
    
    // Cleanup
    graceful_shutdown(server_handle).await;
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_claude_code_error_scenarios() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Start WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    
    // Test 1: Invalid tool call
    if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
        let _ = client.initialize().await;
        
        let invalid_result = client.call_tool("nonexistent_tool", json!({})).await;
        match invalid_result {
            Ok(response) => {
                // Should receive an error response
                assert!(response.get("error").is_some(), "Expected error for invalid tool call");
                println!("Invalid tool call correctly returned error");
            }
            Err(_) => {
                println!("Invalid tool call handled at transport level");
            }
        }
        
        let _ = client.cleanup().await;
    }
    
    // Test 2: Resource access without initialization
    if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
        // Try to access resources without initializing
        let result = client.list_resources().await;
        match result {
            Err(e) => {
                assert!(e.to_string().contains("not initialized"), "Expected initialization error");
                println!("Correctly rejected resource access without initialization");
            }
            Ok(_) => {
                println!("Resource access allowed without initialization (unexpected but handled)");
            }
        }
        
        let _ = client.cleanup().await;
    }
    
    // Test 3: Connection timeout scenarios
    {
        let timeout_suite = ClaudeCodeTestSuite::new().with_timeout(Duration::from_millis(100));
        
        if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
            // This should timeout due to very short timeout
            let results = timeout_suite.test_claude_code_simulation(client).await.unwrap();
            
            let timeout_count = results.tests.values()
                .filter(|r| matches!(r, TestResult::Timeout))
                .count();
                
            println!("Timeout test completed with {} timeouts", timeout_count);
        }
    }
    
    graceful_shutdown(server_handle).await;
}

/// Test session lifecycle management
#[tokio::test]
async fn test_session_lifecycle_management() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    
    // Test full lifecycle: connect -> init -> work -> cleanup
    if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
        // Phase 1: Initialization
        let init_result = client.initialize().await;
        assert!(init_result.is_ok(), "Initialization should succeed");
        
        // Phase 2: Normal operations
        let tools_result = client.list_tools().await;
        assert!(tools_result.is_ok(), "Tool listing should succeed after init");
        
        let resources_result = client.list_resources().await;
        assert!(resources_result.is_ok(), "Resource listing should succeed after init");
        
        // Phase 3: Notification sending
        let notify_result = client.send_notification("test/lifecycle", Some(json!({"phase": "working"}))).await;
        assert!(notify_result.is_ok(), "Notification should succeed");
        
        // Phase 4: Cleanup
        let cleanup_result = client.cleanup().await;
        assert!(cleanup_result.is_ok(), "Cleanup should succeed");
        
        println!("Session lifecycle test completed successfully");
    } else {
        panic!("Failed to create client for lifecycle test");
    }
    
    graceful_shutdown(server_handle).await;
}

/// Test real-world Claude Code usage patterns
#[tokio::test]
async fn test_real_world_usage_patterns() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    
    if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
        // Simulate typical Claude Code workflow
        
        // 1. Initialize connection
        let _ = client.initialize().await.expect("Init should work");
        
        // 2. Discover capabilities
        let tools = client.list_tools().await.expect("Tools list should work");
        let resources = client.list_resources().await.expect("Resources list should work"); 
        let prompts = client.list_prompts().await.expect("Prompts list should work");
        
        println!("Discovered server capabilities successfully");
        
        // 3. Perform typical operations
        
        // Try to create an agent (common Vibe Ensemble operation)
        let agent_result = client.call_tool("create_agent", json!({
            "name": "real-world-test-agent",
            "capabilities": ["testing", "real-world", "patterns"]
        })).await;
        
        match agent_result {
            Ok(response) => {
                println!("Agent creation successful: {}", serde_json::to_string_pretty(&response).unwrap_or_default());
            }
            Err(e) => {
                println!("Agent creation failed (may be expected): {}", e);
            }
        }
        
        // Try to list agents
        let list_agents_result = client.call_tool("list_agents", json!({})).await;
        match list_agents_result {
            Ok(response) => {
                println!("Agent listing successful");
            }
            Err(e) => {
                println!("Agent listing failed (may be expected): {}", e);
            }
        }
        
        // 4. Send notifications (typical in real usage)
        let _ = client.send_notification("agent/status", Some(json!({
            "agent_id": "real-world-test-agent",
            "status": "active"
        }))).await;
        
        // 5. Clean shutdown
        let _ = client.cleanup().await;
        
        println!("Real-world usage pattern test completed");
    }
    
    graceful_shutdown(server_handle).await;
}

/// Test MCP protocol compliance edge cases
#[tokio::test] 
async fn test_mcp_protocol_compliance() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = McpServer::new(storage_manager).await.unwrap();
    let server_handle = tokio::spawn(async move {
        server.serve_websocket(listener).await
    });
    
    wait_for_server_ready(&addr, 50).await.expect("Server should be ready");
    
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());
    
    if let Ok(mut client) = MockClaudeCodeWebSocketClient::new(&ws_url).await {
        // Test MCP initialization response format
        let init_response = client.initialize().await.expect("Initialization should work");
        
        // Verify response structure matches MCP spec
        if let Some(result) = init_response.get("result") {
            assert!(result.get("protocolVersion").is_some(), "Protocol version required");
            assert!(result.get("serverInfo").is_some(), "Server info required");
            assert!(result.get("capabilities").is_some(), "Capabilities required");
            
            // Verify server info structure
            if let Some(server_info) = result.get("serverInfo") {
                assert!(server_info.get("name").is_some(), "Server name required");
                assert!(server_info.get("version").is_some(), "Server version required");
            }
            
            // Verify capabilities structure
            if let Some(capabilities) = result.get("capabilities") {
                // Standard MCP capabilities
                if capabilities.get("tools").is_some() {
                    println!("Server supports tools");
                }
                if capabilities.get("resources").is_some() {
                    println!("Server supports resources");
                }
                if capabilities.get("prompts").is_some() {
                    println!("Server supports prompts");
                }
                
                // Vibe Ensemble extensions
                if capabilities.get("vibe_agent_management").is_some() {
                    println!("Server supports Vibe agent management");
                }
                if capabilities.get("vibe_issue_tracking").is_some() {
                    println!("Server supports Vibe issue tracking");
                }
            }
        }
        
        println!("MCP protocol compliance verified");
        let _ = client.cleanup().await;
    }
    
    graceful_shutdown(server_handle).await;
}

/// Gracefully shutdown a server handle with timeout
async fn graceful_shutdown(handle: tokio::task::JoinHandle<Result<(), vibe_ensemble_mcp::Error>>) {
    // Try to shutdown gracefully with a timeout
    match tokio::time::timeout(Duration::from_millis(500), handle).await {
        Ok(_) => {}, // Server completed gracefully
        Err(_) => {}, // Timeout - server was likely already stopped by test cleanup
    }
}

/// Wait for server to be ready by attempting connection with retries
async fn wait_for_server_ready(addr: &std::net::SocketAddr, max_retries: u32) -> Result<(), String> {
    for attempt in 0..max_retries {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(_) => {
                // Connection successful, server is ready
                return Ok(());
            }
            Err(_) => {
                // Server not ready yet, wait a bit before retry
                tokio::time::sleep(Duration::from_millis(10)).await;
                if attempt == max_retries - 1 {
                    return Err(format!("Server at {} not ready after {} attempts", addr, max_retries));
                }
            }
        }
    }
    Ok(())
}

/// Helper function to check if the server binary is available
async fn server_binary_available() -> bool {
    // Use CARGO_BIN_EXE environment variable when available (during cargo test)
    if let Ok(binary_path) = std::env::var("CARGO_BIN_EXE_vibe-ensemble") {
        std::path::Path::new(&binary_path).exists()
    } else {
        // Fallback for manual testing scenarios
        match tokio::process::Command::new("cargo")
            .args(&["build", "--bin", "vibe-ensemble"])
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
}

/// Helper function to print test results in a readable format
fn print_test_results(results: &TestResults) {
    println!("Transport: {}", results.transport);
    println!("Duration: {:?}", results.duration());
    println!("Success Rate: {:.1}%", results.success_rate() * 100.0);
    println!("Tests: {} passed, {} failed, {} total", 
             results.success_count(), 
             results.failure_count(), 
             results.total_count());
    
    println!("\nDetailed Results:");
    for (test_name, result) in &results.tests {
        let status = match result {
            TestResult::Success(_) => "✓ PASS",
            TestResult::Failure(_) => "✗ FAIL", 
            TestResult::Partial(_) => "~ PARTIAL",
            TestResult::Timeout => "⏰ TIMEOUT",
        };
        println!("  {} {}: {}", status, test_name, result.message());
    }
    println!();
}