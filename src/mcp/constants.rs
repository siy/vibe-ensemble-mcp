/// Centralized constants and helpers for MCP protocol
use serde_json::{json, Value};

/// MCP Protocol Version - single source of truth
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// JSON-RPC envelope builders to ensure consistency
pub struct JsonRpcEnvelopes;

impl JsonRpcEnvelopes {
    /// Create notifications/initialized response
    pub fn initialized() -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        })
    }

    /// Create notifications/message with event data
    pub fn message(event: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/message",
            "params": {
                "event": event
            }
        })
    }

    /// Create notifications/resources/updated
    pub fn resources_updated(uri: &str, event: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {
                "uri": uri,
                "event": event
            }
        })
    }

    /// Create notifications/ping with timestamp
    pub fn ping() -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/ping",
            "params": {
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        })
    }

    /// Create JSON-RPC error response
    pub fn error_response(code: i32, message: &str, id: Option<Value>) -> Value {
        json!({
            "jsonrpc": "2.0",
            "error": {
                "code": code,
                "message": message
            },
            "id": id
        })
    }
}

/// Build MCP config JSON for server endpoints
pub fn build_mcp_config(host: &str, port: u16) -> Value {
    json!({
        "mcpServers": {
            "vibe-ensemble-mcp": {
                "type": "http",
                "url": format!("http://{}:{}/mcp", host, port),
                "protocol_version": MCP_PROTOCOL_VERSION
            },
            "vibe-ensemble-sse": {
                "type": "sse",
                "url": format!("http://{}:{}/sse", host, port),
                "protocol_version": MCP_PROTOCOL_VERSION
            }
        }
    })
}
