use async_trait::async_trait;
use serde_json::{json, Value};

use super::tools::{create_error_response, create_success_response, extract_param, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{database::workers::Worker, error::Result, server::AppState};

pub struct ListWorkersTool;

#[async_trait]
impl ToolHandler for ListWorkersTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: Option<String> =
            super::tools::extract_optional_param(&arguments, "project_id")?;

        match Worker::list_by_project(&state.db, project_id.as_deref()).await {
            Ok(workers) => {
                let workers_json = serde_json::to_string_pretty(&workers)?;
                Ok(create_success_response(&format!(
                    "Workers:\n{}",
                    workers_json
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to list workers: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_workers".to_string(),
            description: "List all workers, optionally filtered by project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to filter workers"
                    }
                }
            }),
        }
    }
}

pub struct FinishWorkerTool;

#[async_trait]
impl ToolHandler for FinishWorkerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;
        let reason: Option<String> = super::tools::extract_optional_param(&arguments, "reason")?;

        // Get worker info to verify it exists
        match Worker::get_by_id(&state.db, &worker_id).await? {
            Some(_) => {
                // Update worker status to finished
                Worker::update_status(&state.db, &worker_id, "finished", None).await?;

                // Create worker stopped event
                crate::database::events::Event::create_worker_stopped(
                    &state.db,
                    &worker_id,
                    &reason.unwrap_or_else(|| "completed all tasks".to_string()),
                )
                .await?;

                Ok(create_success_response(&format!(
                    "Worker '{}' marked as finished successfully",
                    worker_id
                )))
            }
            None => Ok(create_error_response(&format!(
                "Worker '{}' not found",
                worker_id
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "finish_worker".to_string(),
            description:
                "Mark a worker as finished when it completes all tasks and is ready to exit"
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID that is finishing work"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason for finishing (defaults to 'completed all tasks')"
                    }
                },
                "required": ["worker_id"]
            }),
        }
    }
}
