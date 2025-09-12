use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, warn};
use uuid::Uuid;

use super::{
    tools::{create_success_response, extract_optional_param, extract_param, ToolHandler},
    types::{CallToolResponse, Tool, ToolContent},
};
use crate::{
    database::{events::Event, tickets::Ticket, workers::Worker},
    server::AppState,
    workers::{process::ProcessManager, types::SpawnWorkerRequest},
};

pub struct ListEventsTool;

#[async_trait]
impl ToolHandler for ListEventsTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments.unwrap_or_default();

        let event_type: Option<String> = extract_optional_param(&Some(args.clone()), "event_type")?;
        let limit: i32 = extract_optional_param(&Some(args.clone()), "limit")?.unwrap_or(50);

        let events = Event::get_recent(&state.db, limit).await?;

        let filtered_events: Vec<_> = events
            .into_iter()
            .filter(|event| {
                if let Some(ref type_filter) = event_type {
                    &event.event_type == type_filter
                } else {
                    true
                }
            })
            .collect();

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: serde_json::to_string_pretty(&filtered_events)?,
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_events".to_string(),
            description: "List recent system events, optionally filtered by type".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "event_type": {
                        "type": "string",
                        "description": "Optional event type filter (worker_spawned, worker_stopped, ticket_created, etc.)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return",
                        "default": 50
                    }
                },
                "required": []
            }),
        }
    }
}

pub struct GetTaskQueueTool;

#[async_trait]
impl ToolHandler for GetTaskQueueTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let queue_name: String = extract_param(&Some(args.clone()), "queue_name")?;

        info!("Getting tasks from queue: {}", queue_name);

        let tasks = state.queue_manager.get_queue_tasks(&queue_name).await?;

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: serde_json::to_string_pretty(&tasks)?,
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_queue_tasks".to_string(),
            description: "Get all tasks in a specific queue without removing them".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}

pub struct AssignTaskTool;

#[async_trait]
impl ToolHandler for AssignTaskTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let queue_name: String = extract_param(&Some(args.clone()), "queue_name")?;

        info!("Assigning ticket {} to queue {}", ticket_id, queue_name);

        // Create queue if it doesn't exist
        state.queue_manager.create_queue(&queue_name).await?;

        // Check if there's an active worker for this queue
        let has_active_worker = Worker::has_active_worker_for_queue(&state.db, &queue_name).await?;

        if !has_active_worker {
            info!(
                "No active worker found for queue {}. Auto-spawning worker.",
                queue_name
            );

            // Get ticket to find project_id
            if let Some(ticket) = Ticket::get_by_id(&state.db, &ticket_id).await? {
                // Extract worker type from queue name (e.g., "architect-queue" -> "architect")
                let worker_type = if queue_name.ends_with("-queue") {
                    queue_name.strip_suffix("-queue").unwrap_or(&queue_name)
                } else if queue_name.starts_with("queue-") {
                    queue_name.strip_prefix("queue-").unwrap_or(&queue_name)
                } else {
                    &queue_name
                }
                .to_string();

                // Generate unique worker ID
                let worker_id =
                    format!("auto-{}-{}", worker_type, &Uuid::new_v4().to_string()[..8]);

                let spawn_request = SpawnWorkerRequest {
                    worker_id: worker_id.clone(),
                    project_id: ticket.ticket.project_id,
                    worker_type,
                    queue_name: queue_name.clone(),
                };

                match ProcessManager::spawn_worker(state, spawn_request).await {
                    Ok(_worker_process) => {
                        info!("Auto-spawned worker {} for queue {}", worker_id, queue_name);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to auto-spawn worker for queue {}: {}",
                            queue_name, e
                        );
                        // Continue with task assignment even if worker spawn fails
                    }
                }
            } else {
                warn!(
                    "Ticket {} not found, cannot determine project for auto-spawn",
                    ticket_id
                );
            }
        }

        // Add task to queue
        let task_id = state
            .queue_manager
            .add_task(&queue_name, &ticket_id)
            .await?;

        // Create event for task assignment
        Event::create_task_assigned(&state.db, &ticket_id, &queue_name).await?;

        Ok(create_success_response(&format!(
            "Assigned ticket {} to queue {} with task ID: {}{}",
            ticket_id,
            queue_name,
            task_id,
            if !has_active_worker {
                " (auto-spawned worker)"
            } else {
                ""
            }
        )))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "assign_task".to_string(),
            description: "Assign a ticket to a worker queue as a task".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier to assign"
                    },
                    "queue_name": {
                        "type": "string",
                        "description": "Target queue name (should match worker queue)"
                    }
                },
                "required": ["ticket_id", "queue_name"]
            }),
        }
    }
}
