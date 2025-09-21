use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use super::tools::{create_json_success_response, create_json_error_response, extract_param, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{error::Result, server::AppState};

/// Tool for real-time collaboration between clients
pub struct CollaborativeSyncTool;

#[async_trait]
impl ToolHandler for CollaborativeSyncTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let sync_id: String = extract_param(&arguments, "sync_id")?;
        let action: String = extract_param(&arguments, "action")?;
        let data: Value = extract_param(&arguments, "data")?;

        info!("Collaborative sync '{}' action: {}", sync_id, action);

        match action.as_str() {
            "start" => {
                // Notify all clients about sync session start
                let client_ids = state.websocket_manager.list_clients();
                let mut notifications_sent = 0;

                for client_id in client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/sync/started",
                        "params": {
                            "sync_id": sync_id,
                            "data": data
                        }
                    });

                    if state.websocket_manager.send_message(&client_id, &notification).await.is_ok() {
                        notifications_sent += 1;
                    }
                }

                let response = json!({
                    "sync_id": sync_id,
                    "action": "start",
                    "clients_notified": notifications_sent
                });

                Ok(create_json_success_response(response))
            }
            "update" => {
                // Broadcast update to all clients
                let client_ids = state.websocket_manager.list_clients();
                let mut updates_sent = 0;

                for client_id in client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/sync/update",
                        "params": {
                            "sync_id": sync_id,
                            "data": data
                        }
                    });

                    if state.websocket_manager.send_message(&client_id, &notification).await.is_ok() {
                        updates_sent += 1;
                    }
                }

                let response = json!({
                    "sync_id": sync_id,
                    "action": "update",
                    "updates_sent": updates_sent
                });

                Ok(create_json_success_response(response))
            }
            "end" => {
                // Notify all clients about sync session end
                let client_ids = state.websocket_manager.list_clients();
                let mut notifications_sent = 0;

                for client_id in client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/sync/ended",
                        "params": {
                            "sync_id": sync_id,
                            "data": data
                        }
                    });

                    if state.websocket_manager.send_message(&client_id, &notification).await.is_ok() {
                        notifications_sent += 1;
                    }
                }

                let response = json!({
                    "sync_id": sync_id,
                    "action": "end",
                    "clients_notified": notifications_sent
                });

                Ok(create_json_success_response(response))
            }
            _ => Ok(create_json_error_response(&format!("Unknown sync action: {}", action)))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "collaborative_sync".to_string(),
            description: "Manage collaborative sync sessions between connected clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sync_id": {
                        "type": "string",
                        "description": "Unique identifier for the sync session"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["start", "update", "end"],
                        "description": "Sync action to perform"
                    },
                    "data": {
                        "type": "object",
                        "description": "Data to sync between clients"
                    }
                },
                "required": ["sync_id", "action", "data"]
            }),
        }
    }
}

/// Tool for polling clients for status information
pub struct PollClientStatusTool;

#[async_trait]
impl ToolHandler for PollClientStatusTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let timeout_secs = arguments
            .as_ref()
            .and_then(|args| args.get("timeout_secs"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        let client_filter = arguments
            .as_ref()
            .and_then(|args| args.get("client_filter"))
            .and_then(|v| v.as_str());

        info!("Polling client status with timeout: {}s", timeout_secs);

        let client_ids = state.websocket_manager.list_clients();
        let target_clients: Vec<String> = if let Some(filter) = client_filter {
            client_ids.into_iter()
                .filter(|id| id.contains(filter))
                .collect()
        } else {
            client_ids
        };

        let mut status_results = Vec::new();

        for client_id in target_clients {
            // Try to call a status tool on the client
            let status_request = json!({});

            match state.websocket_manager.call_client_tool(&client_id, "get_status", status_request, timeout_secs).await {
                Ok(status) => {
                    status_results.push(json!({
                        "client_id": client_id,
                        "status": "responsive",
                        "data": status
                    }));
                }
                Err(e) => {
                    status_results.push(json!({
                        "client_id": client_id,
                        "status": "error",
                        "error": e.to_string()
                    }));
                }
            }
        }

        let response = json!({
            "total_clients": status_results.len(),
            "timeout_secs": timeout_secs,
            "results": status_results
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "poll_client_status".to_string(),
            description: "Poll connected clients for their status information".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds for each client poll",
                        "default": 10
                    },
                    "client_filter": {
                        "type": "string",
                        "description": "Optional filter string to select specific clients"
                    }
                },
                "required": []
            }),
        }
    }
}

/// Tool for creating dynamic client groups and managing them
pub struct ClientGroupManagerTool;

#[async_trait]
impl ToolHandler for ClientGroupManagerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let action: String = extract_param(&arguments, "action")?;

        match action.as_str() {
            "create_group" => {
                let group_name: String = extract_param(&arguments, "group_name")?;
                let client_ids: Vec<String> = extract_param(&arguments, "client_ids")?;

                // Notify selected clients about group membership
                let mut notifications_sent = 0;
                for client_id in &client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/group/joined",
                        "params": {
                            "group_name": group_name,
                            "members": client_ids
                        }
                    });

                    if state.websocket_manager.send_message(client_id, &notification).await.is_ok() {
                        notifications_sent += 1;
                    }
                }

                let response = json!({
                    "action": "create_group",
                    "group_name": group_name,
                    "member_count": client_ids.len(),
                    "notifications_sent": notifications_sent
                });

                Ok(create_json_success_response(response))
            }
            "broadcast_to_group" => {
                let group_name: String = extract_param(&arguments, "group_name")?;
                let message: Value = extract_param(&arguments, "message")?;
                let client_ids: Vec<String> = extract_param(&arguments, "client_ids")?;

                let mut messages_sent = 0;
                for client_id in &client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/group/message",
                        "params": {
                            "group_name": group_name,
                            "message": message
                        }
                    });

                    if state.websocket_manager.send_message(client_id, &notification).await.is_ok() {
                        messages_sent += 1;
                    }
                }

                let response = json!({
                    "action": "broadcast_to_group",
                    "group_name": group_name,
                    "target_count": client_ids.len(),
                    "messages_sent": messages_sent
                });

                Ok(create_json_success_response(response))
            }
            "dissolve_group" => {
                let group_name: String = extract_param(&arguments, "group_name")?;
                let client_ids: Vec<String> = extract_param(&arguments, "client_ids")?;

                let mut notifications_sent = 0;
                for client_id in &client_ids {
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/group/dissolved",
                        "params": {
                            "group_name": group_name
                        }
                    });

                    if state.websocket_manager.send_message(client_id, &notification).await.is_ok() {
                        notifications_sent += 1;
                    }
                }

                let response = json!({
                    "action": "dissolve_group",
                    "group_name": group_name,
                    "notifications_sent": notifications_sent
                });

                Ok(create_json_success_response(response))
            }
            _ => Ok(create_json_error_response(&format!("Unknown group action: {}", action)))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "manage_client_groups".to_string(),
            description: "Create and manage dynamic groups of connected clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create_group", "broadcast_to_group", "dissolve_group"],
                        "description": "Group management action to perform"
                    },
                    "group_name": {
                        "type": "string",
                        "description": "Name of the group"
                    },
                    "client_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of client IDs in the group"
                    },
                    "message": {
                        "type": "object",
                        "description": "Message to broadcast (for broadcast_to_group action)"
                    }
                },
                "required": ["action", "group_name"]
            }),
        }
    }
}

/// Tool for monitoring and managing client health/heartbeat
pub struct ClientHealthMonitorTool;

#[async_trait]
impl ToolHandler for ClientHealthMonitorTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let action: String = extract_param(&arguments, "action")?;

        match action.as_str() {
            "ping_all" => {
                let client_ids = state.websocket_manager.list_clients();
                let mut ping_results = Vec::new();

                for client_id in client_ids {
                    let ping_notification = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/ping",
                        "params": {
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    });

                    let start_time = std::time::Instant::now();
                    match state.websocket_manager.send_message(&client_id, &ping_notification).await {
                        Ok(_) => {
                            let latency = start_time.elapsed().as_millis();
                            ping_results.push(json!({
                                "client_id": client_id,
                                "status": "ping_sent",
                                "latency_ms": latency
                            }));
                        }
                        Err(e) => {
                            ping_results.push(json!({
                                "client_id": client_id,
                                "status": "failed",
                                "error": e.to_string()
                            }));
                        }
                    }
                }

                let response = json!({
                    "action": "ping_all",
                    "total_clients": ping_results.len(),
                    "results": ping_results
                });

                Ok(create_json_success_response(response))
            }
            "health_check" => {
                let client_ids = state.websocket_manager.list_clients();
                let mut health_results = Vec::new();

                for client_id in &client_ids {
                    // Get client connection info
                    if let Some(client_entry) = state.websocket_manager.clients.get(client_id) {
                        let client = client_entry.value();
                        let connection_duration = chrono::Utc::now()
                            .signed_duration_since(client.connected_at)
                            .num_seconds();

                        health_results.push(json!({
                            "client_id": client_id,
                            "connected_at": client.connected_at.to_rfc3339(),
                            "connection_duration_secs": connection_duration,
                            "capabilities": client.capabilities
                        }));
                    }
                }

                let response = json!({
                    "action": "health_check",
                    "total_clients": health_results.len(),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "clients": health_results
                });

                Ok(create_json_success_response(response))
            }
            _ => Ok(create_json_error_response(&format!("Unknown health monitor action: {}", action)))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "monitor_client_health".to_string(),
            description: "Monitor and check health of connected clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["ping_all", "health_check"],
                        "description": "Health monitoring action to perform"
                    }
                },
                "required": ["action"]
            }),
        }
    }
}