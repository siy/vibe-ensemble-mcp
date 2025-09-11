use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use crate::{error::Result, server::AppState};
use super::tools::{
    ToolHandler, extract_param, create_success_response, create_error_response
};
use super::types::{CallToolResponse, Tool};

pub struct CreateQueueTool;

#[async_trait]
impl ToolHandler for CreateQueueTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        info!("Creating queue: {}", queue_name);

        state.queue_manager.create_queue(&queue_name).await?;

        Ok(create_success_response(&format!("Queue '{}' created successfully", queue_name)))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_queue".to_string(),
            description: "Create a new task queue for workers".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue to create"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}

pub struct ListQueuesTool;

#[async_trait]
impl ToolHandler for ListQueuesTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        match state.queue_manager.list_queues().await {
            Ok(queues) => {
                let queues_json = serde_json::to_string_pretty(&queues)?;
                Ok(create_success_response(&format!("Queues:\n{}", queues_json)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to list queues: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_queues".to_string(),
            description: "List all task queues with their status".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

pub struct GetQueueStatusTool;

#[async_trait]
impl ToolHandler for GetQueueStatusTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        match state.queue_manager.get_queue_status(&queue_name).await {
            Ok(Some(status)) => {
                let status_json = serde_json::to_string_pretty(&status)?;
                Ok(create_success_response(&format!("Queue status:\n{}", status_json)))
            }
            Ok(None) => Ok(create_error_response(&format!("Queue '{}' not found", queue_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to get queue status: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_queue_status".to_string(),
            description: "Get the status of a specific queue".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue to check"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}

pub struct DeleteQueueTool;

#[async_trait]
impl ToolHandler for DeleteQueueTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        match state.queue_manager.delete_queue(&queue_name).await {
            Ok(true) => Ok(create_success_response(&format!("Queue '{}' deleted successfully", queue_name))),
            Ok(false) => Ok(create_error_response(&format!("Queue '{}' not found", queue_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to delete queue: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_queue".to_string(),
            description: "Delete a task queue".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue to delete"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}