/// Centralized constants and helpers for MCP protocol
use serde_json::{json, Value};

/// MCP Protocol Version - single source of truth
pub const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

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

    /// Create notifications/sync/started
    pub fn sync_started(sync_id: &str, data: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/sync/started",
            "params": {
                "sync_id": sync_id,
                "data": data
            }
        })
    }

    /// Create notifications/sync/update
    pub fn sync_update(sync_id: &str, data: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/sync/update",
            "params": {
                "sync_id": sync_id,
                "data": data
            }
        })
    }

    /// Create notifications/sync/completed
    pub fn sync_completed(sync_id: &str, data: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/sync/completed",
            "params": {
                "sync_id": sync_id,
                "data": data
            }
        })
    }

    /// Create notifications/sync/ended
    pub fn sync_ended(sync_id: &str, data: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/sync/ended",
            "params": {
                "sync_id": sync_id,
                "data": data
            }
        })
    }

    /// Create notifications/group/joined
    pub fn group_joined(group_name: &str, members: Vec<String>) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/group/joined",
            "params": {
                "group_name": group_name,
                "members": members
            }
        })
    }

    /// Create notifications/group/message
    pub fn group_message(group_name: &str, message: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/group/message",
            "params": {
                "group_name": group_name,
                "message": message
            }
        })
    }

    /// Create notifications/group/dissolved
    pub fn group_dissolved(group_name: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/group/dissolved",
            "params": {
                "group_name": group_name
            }
        })
    }

    /// Create custom notification with method and params
    pub fn notification(method: &str, params: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
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
            },
            "vibe-ensemble-ws": {
                "type": "websocket",
                "url": format!("ws://{}:{}/ws", host, port),
                "protocol_version": MCP_PROTOCOL_VERSION,
                "bidirectional": true,
                "features": [
                    "server_initiated_requests",
                    "client_tool_registration",
                    "real_time_collaboration",
                    "workflow_orchestration"
                ]
            }
        }
    })
}
