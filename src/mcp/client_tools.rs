use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::tools::{
    create_json_error_response, create_json_success_response, extract_param, ToolHandler,
};
use super::types::{CallToolResponse, Tool};
use crate::{error::Result, server::AppState};

/// Tool for listing all available client tools from connected clients
pub struct ListClientToolsTool;

#[async_trait]
impl ToolHandler for ListClientToolsTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        let client_tools = state.websocket_manager.tool_registry().list_tools();

        let tools_json: Vec<Value> = client_tools
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                    "client_id": tool.client_id,
                    "registered_at": tool.registered_at.to_rfc3339()
                })
            })
            .collect();

        let response = json!({
            "tools": tools_json,
            "total_count": tools_json.len()
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_client_tools".to_string(),
            description: "List all tools available from connected clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

/// Tool for calling a specific client tool
pub struct CallClientToolTool;

#[async_trait]
impl ToolHandler for CallClientToolTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let client_id: String = extract_param(&arguments, "client_id")?;
        let tool_name: String = extract_param(&arguments, "tool_name")?;
        let tool_arguments = arguments
            .as_ref()
            .and_then(|args| args.get("arguments"))
            .cloned()
            .unwrap_or_else(|| json!({}));

        info!(
            "Calling client tool: {} on client: {}",
            tool_name, client_id
        );

        // Verify the tool exists
        if state
            .websocket_manager
            .tool_registry()
            .get_tool(&client_id, &tool_name)
            .is_none()
        {
            return Ok(create_json_error_response(&format!(
                "Tool '{}' not found on client '{}'",
                tool_name, client_id
            )));
        }

        // Call the client tool
        match state
            .websocket_manager
            .call_client_tool(
                &client_id,
                &tool_name,
                tool_arguments,
                state.config.client_tool_timeout_secs,
            )
            .await
        {
            Ok(result) => {
                info!(
                    "Client tool call successful: {} -> {}",
                    tool_name, client_id
                );
                let response = json!({
                    "client_id": client_id,
                    "tool_name": tool_name,
                    "result": result
                });
                Ok(create_json_success_response(response))
            }
            Err(e) => {
                warn!(
                    "Client tool call failed: {} -> {} - {}",
                    tool_name, client_id, e
                );
                Ok(create_json_error_response(&format!(
                    "Failed to call tool '{}' on client '{}': {}",
                    tool_name, client_id, e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "call_client_tool".to_string(),
            description: "Call a tool on a connected client".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "client_id": {
                        "type": "string",
                        "description": "ID of the client to call the tool on"
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to call"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool"
                    }
                },
                "required": ["client_id", "tool_name"]
            }),
        }
    }
}

/// Tool for listing connected clients and their capabilities
pub struct ListConnectedClientsTool;

#[async_trait]
impl ToolHandler for ListConnectedClientsTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        let client_ids = state.websocket_manager.list_clients();

        let clients_info: Vec<Value> = client_ids
            .into_iter()
            .filter_map(|client_id| {
                state
                    .websocket_manager
                    .clients
                    .get(&client_id)
                    .map(|entry| {
                        let client = entry.value();
                        json!({
                            "client_id": client.client_id,
                            "connected_at": client.connected_at.to_rfc3339(),
                            "capabilities": {
                                "bidirectional": client.capabilities.bidirectional,
                                "tools_count": client.capabilities.tools.len(),
                                "client_info": {
                                    "name": client.capabilities.client_info.name,
                                    "version": client.capabilities.client_info.version,
                                    "environment": client.capabilities.client_info.environment
                                }
                            }
                        })
                    })
            })
            .collect();

        let response = json!({
            "clients": clients_info,
            "total_count": clients_info.len()
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_connected_clients".to_string(),
            description: "List all connected WebSocket clients and their capabilities".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

/// Tool for getting detailed information about pending requests
pub struct ListPendingRequestsTool;

#[async_trait]
impl ToolHandler for ListPendingRequestsTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        let pending_requests = state.websocket_manager.pending_requests();

        let requests_info: Vec<Value> = pending_requests
            .iter()
            .map(|entry| {
                let (request_id, pending) = entry.pair();
                json!({
                    "request_id": request_id,
                    "client_id": pending.client_id,
                    "tool_name": pending.tool_name,
                    "created_at": pending.created_at.to_rfc3339()
                })
            })
            .collect();

        let response = json!({
            "pending_requests": requests_info,
            "total_count": requests_info.len()
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_pending_requests".to_string(),
            description: "List all pending server-initiated requests to clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}
