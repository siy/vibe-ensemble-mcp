use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info, warn};

use super::tools::{create_json_success_response, create_json_error_response, extract_param, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{error::Result, server::AppState};

/// Tool for executing a workflow of client tool calls
pub struct ExecuteWorkflowTool;

#[async_trait]
impl ToolHandler for ExecuteWorkflowTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let workflow: Vec<Value> = extract_param(&arguments, "workflow")?;
        let workflow_id: String = extract_param(&arguments, "workflow_id")?;

        info!("Executing workflow '{}' with {} steps", workflow_id, workflow.len());

        let mut results = Vec::new();
        let mut context: HashMap<String, Value> = HashMap::new();

        for (step_index, step) in workflow.iter().enumerate() {
            let default_name = format!("step_{}", step_index);
            let step_name = step.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&default_name);

            let client_id = step.get("client_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| crate::error::AppError::BadRequest(
                    format!("Missing client_id in step {}", step_index)
                ))?;

            let tool_name = step.get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| crate::error::AppError::BadRequest(
                    format!("Missing tool_name in step {}", step_index)
                ))?;

            // Process arguments with context substitution
            let mut step_arguments = step.get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            // Simple context substitution for ${variable} patterns
            if let Some(arguments_obj) = step_arguments.as_object_mut() {
                for (_key, value) in arguments_obj.iter_mut() {
                    if let Some(str_value) = value.as_str() {
                        if str_value.starts_with("${") && str_value.ends_with("}") {
                            let var_name = &str_value[2..str_value.len()-1];
                            if let Some(context_value) = context.get(var_name) {
                                *value = context_value.clone();
                            }
                        }
                    }
                }
            }

            info!("Executing step {}: {} -> {}", step_index, tool_name, client_id);

            // Execute the tool call
            match state.websocket_manager.call_client_tool(client_id, tool_name, step_arguments, state.config.client_tool_timeout_secs).await {
                Ok(result) => {
                    info!("Step {} completed successfully", step_index);

                    // Store result in context for future steps
                    context.insert(format!("step_{}_result", step_index), result.clone());
                    if let Some(name) = step.get("name").and_then(|v| v.as_str()) {
                        context.insert(format!("{}_result", name), result.clone());
                    }

                    results.push(json!({
                        "step": step_index,
                        "name": step_name,
                        "status": "success",
                        "result": result
                    }));
                }
                Err(e) => {
                    error!("Step {} failed: {}", step_index, e);

                    // Check if this step is marked as optional
                    let is_optional = step.get("optional")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    results.push(json!({
                        "step": step_index,
                        "name": step_name,
                        "status": "failed",
                        "error": e.to_string(),
                        "optional": is_optional
                    }));

                    if !is_optional {
                        warn!("Workflow '{}' failed at step {}: {}", workflow_id, step_index, e);
                        return Ok(create_json_error_response(&format!(
                            "Workflow failed at step {}: {}", step_index, e
                        )));
                    } else {
                        warn!("Optional step {} failed, continuing workflow", step_index);
                    }
                }
            }
        }

        info!("Workflow '{}' completed successfully", workflow_id);

        let response = json!({
            "workflow_id": workflow_id,
            "status": "completed",
            "steps_executed": results.len(),
            "results": results,
            "context": context
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "execute_workflow".to_string(),
            description: "Execute a workflow of client tool calls with context passing".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "Unique identifier for this workflow execution"
                    },
                    "workflow": {
                        "type": "array",
                        "description": "Array of workflow steps to execute",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Optional name for this step"
                                },
                                "client_id": {
                                    "type": "string",
                                    "description": "ID of the client to call"
                                },
                                "tool_name": {
                                    "type": "string",
                                    "description": "Name of the tool to call"
                                },
                                "arguments": {
                                    "type": "object",
                                    "description": "Arguments for the tool call (supports ${variable} substitution)"
                                },
                                "optional": {
                                    "type": "boolean",
                                    "description": "Whether this step is optional (won't fail the workflow)"
                                }
                            },
                            "required": ["client_id", "tool_name"]
                        }
                    }
                },
                "required": ["workflow_id", "workflow"]
            }),
        }
    }
}

/// Tool for calling multiple client tools in parallel
pub struct ParallelCallTool;

#[async_trait]
impl ToolHandler for ParallelCallTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let calls: Vec<Value> = extract_param(&arguments, "calls")?;
        let call_id: String = extract_param(&arguments, "call_id")?;

        info!("Executing {} parallel client tool calls for '{}'", calls.len(), call_id);

        let mut futures = Vec::new();

        for (index, call) in calls.iter().enumerate() {
            let client_id = call.get("client_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| crate::error::AppError::BadRequest(
                    format!("Missing client_id in call {}", index)
                ))?;

            let tool_name = call.get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| crate::error::AppError::BadRequest(
                    format!("Missing tool_name in call {}", index)
                ))?;

            let tool_arguments = call.get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let ws_manager = state.websocket_manager.clone();
            let client_id = client_id.to_string();
            let tool_name = tool_name.to_string();
            let timeout_secs = state.config.client_tool_timeout_secs;

            let future = async move {
                let result = ws_manager.call_client_tool(&client_id, &tool_name, tool_arguments, timeout_secs).await;
                (index, client_id, tool_name, result)
            };

            futures.push(future);
        }

        // Execute all calls in parallel
        let results = futures_util::future::join_all(futures).await;

        let mut call_results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        for (index, client_id, tool_name, result) in results {
            match result {
                Ok(value) => {
                    success_count += 1;
                    call_results.push(json!({
                        "index": index,
                        "client_id": client_id,
                        "tool_name": tool_name,
                        "status": "success",
                        "result": value
                    }));
                }
                Err(e) => {
                    failure_count += 1;
                    call_results.push(json!({
                        "index": index,
                        "client_id": client_id,
                        "tool_name": tool_name,
                        "status": "failed",
                        "error": e.to_string()
                    }));
                }
            }
        }

        info!("Parallel calls completed: {} succeeded, {} failed", success_count, failure_count);

        let response = json!({
            "call_id": call_id,
            "total_calls": calls.len(),
            "success_count": success_count,
            "failure_count": failure_count,
            "results": call_results
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "parallel_call_client_tools".to_string(),
            description: "Call multiple client tools in parallel".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "call_id": {
                        "type": "string",
                        "description": "Unique identifier for this parallel call batch"
                    },
                    "calls": {
                        "type": "array",
                        "description": "Array of tool calls to execute in parallel",
                        "items": {
                            "type": "object",
                            "properties": {
                                "client_id": {
                                    "type": "string",
                                    "description": "ID of the client to call"
                                },
                                "tool_name": {
                                    "type": "string",
                                    "description": "Name of the tool to call"
                                },
                                "arguments": {
                                    "type": "object",
                                    "description": "Arguments for the tool call"
                                }
                            },
                            "required": ["client_id", "tool_name"]
                        }
                    }
                },
                "required": ["call_id", "calls"]
            }),
        }
    }
}

/// Tool for broadcasting a message to all connected clients
pub struct BroadcastToClientsTool;

#[async_trait]
impl ToolHandler for BroadcastToClientsTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let message: Value = extract_param(&arguments, "message")?;
        let client_filter = arguments
            .as_ref()
            .and_then(|args| args.get("client_filter"))
            .and_then(|v| v.as_str());

        let client_ids = state.websocket_manager.list_clients();

        // Filter clients if a filter is provided
        let target_clients: Vec<String> = if let Some(filter) = client_filter {
            client_ids.into_iter()
                .filter(|id| id.contains(filter))
                .collect()
        } else {
            client_ids
        };

        info!("Broadcasting message to {} clients", target_clients.len());

        let mut results = Vec::new();
        for client_id in target_clients {
            let notification = json!({
                "jsonrpc": "2.0",
                "method": "notifications/message",
                "params": {
                    "message": message
                }
            });

            match state.websocket_manager.send_message(&client_id, &notification).await {
                Ok(_) => {
                    results.push(json!({
                        "client_id": client_id,
                        "status": "sent"
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "client_id": client_id,
                        "status": "failed",
                        "error": e.to_string()
                    }));
                }
            }
        }

        let response = json!({
            "message_sent": true,
            "target_count": results.len(),
            "results": results
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "broadcast_to_clients".to_string(),
            description: "Broadcast a message to all or filtered connected clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "object",
                        "description": "Message to broadcast to clients"
                    },
                    "client_filter": {
                        "type": "string",
                        "description": "Optional filter string to select specific clients"
                    }
                },
                "required": ["message"]
            }),
        }
    }
}