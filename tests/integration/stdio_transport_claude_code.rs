//! Integration tests for stdio transport Claude Code compatibility
//!
//! These tests verify that the enhanced stdio transport meets all Claude Code
//! requirements for MCP protocol communication over stdin/stdout.

use serde_json::{json, Value};
use std::time::Duration;
use tempfile::NamedTempFile;

use vibe_ensemble_mcp::transport::{StdioTransport, Transport, TransportFactory};

/// Test helper to create a temporary database for testing
async fn create_test_db() -> Result<String, Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let db_path = format!("sqlite:{}", temp_file.path().display());
    
    // Keep the file alive by leaking it (for test duration)
    std::mem::forget(temp_file);
    
    Ok(db_path)
}

/// Test JSON-RPC 2.0 message framing compliance
#[tokio::test]
async fn test_json_rpc_message_framing() {
    // Test that messages are properly newline-delimited
    let valid_messages = vec![
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"test","version":"1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"notification","params":{"message":"test"}}"#,
        r#"{"jsonrpc":"2.0","result":{"capabilities":{}},"id":1}"#,
    ];

    for message in valid_messages {
        assert!(StdioTransport::validate_message(message).is_ok(), 
                "Message should be valid: {}", message);
    }

    // Test invalid messages
    let invalid_messages = vec![
        "not json at all",
        r#"{"jsonrpc":"1.0","id":1}"#,  // Wrong version
        "{\"jsonrpc\":\"2.0\",\n\"id\":1}",  // Embedded newline
        "{\"jsonrpc\":\"2.0\",\r\"id\":1}",  // Embedded carriage return
        r#"{"id":1,"method":"test"}"#,  // Missing jsonrpc field (strict validation)
        "[]",  // Empty batch
        "\"just a string\"",  // Non-object/array root
    ];

    for message in invalid_messages {
        assert!(StdioTransport::validate_message(message).is_err(),
                "Message should be invalid: {}", message);
    }
}

/// Test stdio transport performance with large messages
#[tokio::test]
async fn test_large_message_handling() {
    // Create a large but valid JSON-RPC message
    let large_params = (0..1000).map(|i| format!("item_{}", i)).collect::<Vec<_>>();
    let large_message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "test_large",
        "params": {
            "data": large_params
        }
    }).to_string();

    // Validate the large message
    assert!(StdioTransport::validate_message(&large_message).is_ok());
    assert!(large_message.len() > 10000, "Message should be large enough for testing");
    
    // Test with high-performance buffer configuration
    let _transport = TransportFactory::stdio_with_config(
        Duration::from_secs(60),
        Duration::from_secs(30),
        256 * 1024,  // 256KB buffer for large messages
    );
}

/// Test timeout handling for stdio operations
#[tokio::test]
async fn test_timeout_handling() {
    use tokio::time::Duration;
    
    // Test that timeout constants are reasonable for Claude Code usage
    assert!(StdioTransport::DEFAULT_READ_TIMEOUT >= Duration::from_secs(10));
    assert!(StdioTransport::DEFAULT_WRITE_TIMEOUT >= Duration::from_secs(5));
    
    // Test custom timeout configuration
    let short_timeout_transport = StdioTransport::with_config(
        Duration::from_millis(100),  // Very short timeout for testing
        Duration::from_millis(100),
        1024,
    );
    
    assert_eq!(short_timeout_transport.read_timeout, Duration::from_millis(100));
    assert_eq!(short_timeout_transport.write_timeout, Duration::from_millis(100));
}

/// Test buffer size optimization for different use cases
#[tokio::test]
async fn test_buffer_optimization() {
    // Test default buffer size is reasonable
    assert!(StdioTransport::DEFAULT_BUFFER_SIZE >= 8192); // At least 8KB
    assert!(StdioTransport::DEFAULT_BUFFER_SIZE <= 1024 * 1024); // At most 1MB
    
    // Test various buffer configurations
    let configs = vec![
        (4096, "Small buffer for embedded systems"),
        (64 * 1024, "Default buffer for normal usage"),  
        (256 * 1024, "Large buffer for high-throughput"),
        (1024 * 1024, "Very large buffer for bulk operations"),
    ];
    
    for (buffer_size, description) in configs {
        let transport = StdioTransport::with_config(
            Duration::from_secs(30),
            Duration::from_secs(10),
            buffer_size,
        );
        assert_eq!(transport.buffer_size, buffer_size, "{}", description);
    }
}

/// Test Unicode encoding compliance 
#[tokio::test]
async fn test_unicode_encoding_compliance() {
    let unicode_test_cases = vec![
        ("ASCII only", r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#),
        ("Latin-1", r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"Café"}}"#),
        ("Chinese", r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"你好世界"}}"#),
        ("Japanese", r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"こんにちは"}}"#),
        ("Emoji", r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"Hello 🌍 World 🚀"}}"#),
        ("Mixed", r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"Hello 世界 🌍 Café"}}"#),
    ];

    for (description, message) in unicode_test_cases {
        assert!(StdioTransport::validate_message(message).is_ok(),
                "Unicode message should be valid ({}): {}", description, message);
        
        // Note: All &str in Rust are guaranteed to be valid UTF-8
    }
}

/// Test error recovery and resilience
#[tokio::test]
async fn test_error_recovery() {
    // Test that transport can handle and recover from various error conditions
    
    // Test handling of malformed JSON (should log and continue)
    let malformed_cases = vec![
        "",  // Empty string
        "   ",  // Whitespace only
        "not json",
        "{incomplete json",
        r#"{"jsonrpc":"2.0"}"#,  // Missing required fields
    ];
    
    for malformed in malformed_cases {
        let result = StdioTransport::validate_message(malformed);
        // All these should fail validation but not crash
        assert!(result.is_err(), "Malformed message should fail validation: '{}'", malformed);
    }
}

/// Test Claude Code specific MCP protocol compliance
#[tokio::test]
async fn test_claude_code_mcp_compliance() {
    // Test MCP protocol messages that Claude Code specifically uses
    
    let claude_code_messages = vec![
        // Initialize request
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"claude-code","version":"1.0.0"}}}"#,
        
        // Tools list request
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        
        // Tool call request
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_agent","arguments":{"name":"test-agent","capabilities":["testing"]}}}"#,
        
        // Resources list request
        r#"{"jsonrpc":"2.0","id":4,"method":"resources/list","params":{}}"#,
        
        // Resources read request
        r#"{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"vibe://agents/test-agent"}}"#,
        
        // Prompts list request
        r#"{"jsonrpc":"2.0","id":6,"method":"prompts/list","params":{}}"#,
        
        // Notification (no response expected)
        r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
    ];

    for message in claude_code_messages {
        assert!(StdioTransport::validate_message(message).is_ok(),
                "Claude Code MCP message should be valid: {}", message);
                
        // Verify it's proper JSON
        let parsed: Value = serde_json::from_str(message).expect("Should parse as JSON");
        
        // Verify JSON-RPC 2.0 compliance
        assert_eq!(parsed.get("jsonrpc").and_then(|v| v.as_str()).unwrap_or(""), "2.0");
        
        // Verify method exists for requests
        if parsed.get("id").is_some() {
            assert!(parsed.get("method").is_some(), "Request should have method");
        }
    }
}

/// Test configuration edge cases
#[tokio::test]
async fn test_configuration_edge_cases() {
    use tokio::time::Duration;
    
    // Test minimum viable configuration
    let min_config = StdioTransport::with_config(
        Duration::from_millis(1),  // Minimum timeout
        Duration::from_millis(1),
        64,  // Minimum buffer size
    );
    assert!(!min_config.is_closed);
    
    // Test maximum reasonable configuration
    let max_config = StdioTransport::with_config(
        Duration::from_secs(300),  // 5 minute timeout
        Duration::from_secs(60),   // 1 minute write timeout
        10 * 1024 * 1024,          // 10MB buffer
    );
    assert!(!max_config.is_closed);
}

/// Test signal handling capability
#[tokio::test]
async fn test_signal_handling_setup() {
    // Test that signal handling functions exist and can be called
    // Note: We can't easily test actual signal delivery in unit tests
    
    #[cfg(unix)]
    {
        // On Unix systems, we should be able to set up signal handlers
        use tokio::time::timeout;
        
        // Test that wait_for_sigterm doesn't block indefinitely
        let sigterm_future = StdioTransport::wait_for_sigterm();
        let result = timeout(Duration::from_millis(10), sigterm_future).await;
        assert!(result.is_err(), "SIGTERM wait should timeout in test environment");
    }
    
    #[cfg(not(unix))]
    {
        // On Windows, wait_for_sigterm should be a no-op
        let sigterm_future = StdioTransport::wait_for_sigterm();
        let result = timeout(Duration::from_millis(10), sigterm_future).await;
        assert!(result.is_err(), "SIGTERM wait should timeout on Windows");
    }
}

