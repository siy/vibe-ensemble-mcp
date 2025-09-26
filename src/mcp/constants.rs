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

    /// Create sampling/createMessage notification for Claude to process realtime events
    pub fn sampling_create_message() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sampling/createMessage",
            "params": {
                "messages": [
                    {
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": "Address realtime events and process them."
                        }
                    }
                ],
                "includeContext": "thisServer",
                "maxTokens": 200
            }
        })
    }
}

/// Complete list of MCP tools available on the server
/// This must be kept in sync with the tools registered in server.rs
pub fn get_all_mcp_tool_names() -> Vec<String> {
    vec![
        // Project management tools
        "mcp__vibe-ensemble-mcp__create_project".to_string(),
        "mcp__vibe-ensemble-mcp__list_projects".to_string(),
        "mcp__vibe-ensemble-mcp__get_project".to_string(),
        "mcp__vibe-ensemble-mcp__update_project".to_string(),
        "mcp__vibe-ensemble-mcp__delete_project".to_string(),
        // Worker type management tools
        "mcp__vibe-ensemble-mcp__create_worker_type".to_string(),
        "mcp__vibe-ensemble-mcp__list_worker_types".to_string(),
        "mcp__vibe-ensemble-mcp__get_worker_type".to_string(),
        "mcp__vibe-ensemble-mcp__update_worker_type".to_string(),
        "mcp__vibe-ensemble-mcp__delete_worker_type".to_string(),
        // Ticket management tools
        "mcp__vibe-ensemble-mcp__create_ticket".to_string(),
        "mcp__vibe-ensemble-mcp__get_ticket".to_string(),
        "mcp__vibe-ensemble-mcp__list_tickets".to_string(),
        "mcp__vibe-ensemble-mcp__add_ticket_comment".to_string(),
        "mcp__vibe-ensemble-mcp__close_ticket".to_string(),
        "mcp__vibe-ensemble-mcp__resume_ticket_processing".to_string(),
        // Dependency management tools
        "mcp__vibe-ensemble-mcp__add_ticket_dependency".to_string(),
        "mcp__vibe-ensemble-mcp__remove_ticket_dependency".to_string(),
        "mcp__vibe-ensemble-mcp__get_dependency_graph".to_string(),
        "mcp__vibe-ensemble-mcp__list_ready_tickets".to_string(),
        "mcp__vibe-ensemble-mcp__list_blocked_tickets".to_string(),
        // Event and stage management tools
        "mcp__vibe-ensemble-mcp__list_events".to_string(),
        "mcp__vibe-ensemble-mcp__resolve_event".to_string(),
        "mcp__vibe-ensemble-mcp__get_tickets_by_stage".to_string(),
        // Permission management tools
        "mcp__vibe-ensemble-mcp__get_permission_model".to_string(),
        // Template management tools
        "mcp__vibe-ensemble-mcp__list_worker_templates".to_string(),
        "mcp__vibe-ensemble-mcp__load_worker_template".to_string(),
        "mcp__vibe-ensemble-mcp__ensure_worker_templates_exist".to_string(),
    ]
}

/// Build MCP config JSON for server endpoints
pub fn build_mcp_config(host: &str, port: u16) -> Value {
    json!({
        "mcpServers": {
            "vibe-ensemble-mcp": {
                "type": "http",
                "url": format!("http://{}:{}/mcp", host, port),
                "protocol_version": MCP_PROTOCOL_VERSION
            }
        }
    })
}

/// Build Claude Code permissions configuration with explicit tool names
pub fn build_claude_permissions() -> Value {
    let mut tool_names = get_all_mcp_tool_names();

    // Add essential tools for workers
    tool_names.extend([
        "TodoWrite".to_string(),
        "Bash".to_string(),
        "Read".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "MultiEdit".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
    ]);

    json!({
        "permissions": {
            "allow": tool_names,
            "deny": [
                "WebFetch",
                "WebSearch"
            ],
            "ask": [],
            "defaultMode": "acceptEdits",
            "additionalDirectories": []
        },
        "enableAllProjectMcpServers": true,
        "enabledMcpjsonServers": [
            "vibe-ensemble-mcp"
        ]
    })
}
