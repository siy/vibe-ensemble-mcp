use async_trait::async_trait;
use serde_json::{json, Value};

use super::tools::{create_error_response, create_success_response, extract_param, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{
    database::workers::Worker,
    error::Result,
    server::AppState,
    workers::{process::ProcessManager, types::SpawnWorkerRequest},
};

pub struct SpawnWorkerTool;

#[async_trait]
impl ToolHandler for SpawnWorkerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        let request = SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id,
            worker_type,
            queue_name: queue_name.clone(),
        };

        match ProcessManager::spawn_worker(state, request).await {
            Ok(worker_process) => {
                // Create queue for the worker (using the specified queue name)
                if let Err(e) = state.queue_manager.create_queue(&queue_name).await {
                    return Ok(create_error_response(&format!(
                        "Worker spawned but failed to create queue '{}': {}",
                        queue_name, e
                    )));
                }

                let response = json!({
                    "worker_id": worker_process.info.worker_id,
                    "status": worker_process.info.status.as_str(),
                    "pid": worker_process.info.pid,
                    "queue_name": worker_process.info.queue_name
                });
                Ok(create_success_response(&format!(
                    "Worker spawned successfully: {}",
                    response
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to spawn worker: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "spawn_worker".to_string(),
            description: "Spawn a new worker process with a specific queue for task management"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Unique worker ID (format: worker_<type>_<number>)"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier"
                    },
                    "queue_name": {
                        "type": "string",
                        "description": "Queue name for this worker to pull tasks from (e.g., 'development', 'testing', 'review')"
                    }
                },
                "required": ["worker_id", "project_id", "worker_type", "queue_name"]
            }),
        }
    }
}

pub struct StopWorkerTool;

#[async_trait]
impl ToolHandler for StopWorkerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;

        match ProcessManager::stop_worker(state, &worker_id).await {
            Ok(true) => Ok(create_success_response(&format!(
                "Worker '{}' stopped successfully",
                worker_id
            ))),
            Ok(false) => Ok(create_error_response(&format!(
                "Worker '{}' not found",
                worker_id
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to stop worker: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "stop_worker".to_string(),
            description: "Stop a running worker process".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID to stop"
                    }
                },
                "required": ["worker_id"]
            }),
        }
    }
}

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

pub struct GetWorkerStatusTool;

#[async_trait]
impl ToolHandler for GetWorkerStatusTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;

        match ProcessManager::check_worker_health(state, &worker_id).await {
            Ok(status) => {
                let worker = Worker::get_by_id(&state.db, &worker_id).await?;
                match worker {
                    Some(worker) => {
                        let response = json!({
                            "worker_id": worker.worker_id,
                            "status": status.as_str(),
                            "pid": worker.pid,
                            "last_activity": worker.last_activity
                        });
                        Ok(create_success_response(&format!(
                            "Worker status: {}",
                            response
                        )))
                    }
                    None => Ok(create_error_response(&format!(
                        "Worker '{}' not found",
                        worker_id
                    ))),
                }
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to get worker status: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_worker_status".to_string(),
            description: "Get the current status of a worker".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID to check"
                    }
                },
                "required": ["worker_id"]
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
