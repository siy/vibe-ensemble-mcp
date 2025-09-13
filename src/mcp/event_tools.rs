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

pub struct GetTicketsByStageTool;

#[async_trait]
impl ToolHandler for GetTicketsByStageTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let stage: String = extract_param(&Some(args.clone()), "stage")?;

        info!("Getting tickets for stage: {}", stage);

        // Get tickets with matching current_stage
        let tickets = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at
            FROM tickets
            WHERE current_stage = ?1 AND state = 'open'
            ORDER BY 
                CASE priority 
                    WHEN 'urgent' THEN 1
                    WHEN 'high' THEN 2  
                    WHEN 'medium' THEN 3
                    WHEN 'low' THEN 4
                    ELSE 5
                END,
                created_at ASC
        "#,
        )
        .bind(&stage)
        .fetch_all(&state.db)
        .await?;

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: serde_json::to_string_pretty(&tickets)?,
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_tickets_by_stage".to_string(),
            description: "Get all open tickets currently in a specific stage, ordered by priority"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stage": {
                        "type": "string",
                        "description": "Name of the stage (e.g., 'planning', 'design', 'coding', 'testing')"
                    }
                },
                "required": ["stage"]
            }),
        }
    }
}

pub struct SpawnWorkerForStageTool;

#[async_trait]
impl ToolHandler for SpawnWorkerForStageTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let stage: String = extract_param(&Some(args.clone()), "stage")?;
        let project_id: String = extract_param(&Some(args.clone()), "project_id")?;

        info!(
            "Spawning worker for stage: {} in project: {}",
            stage, project_id
        );

        // Check if there's already an active worker for this stage
        let existing_workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity
            FROM workers 
            WHERE project_id = ?1 AND worker_type = ?2 AND status IN ('spawning', 'active', 'idle')
        "#,
        )
        .bind(&project_id)
        .bind(&stage)
        .fetch_all(&state.db)
        .await?;

        // Check if any existing workers are actually running
        for worker in &existing_workers {
            if let Some(pid) = worker.pid {
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);

                if is_running {
                    return Ok(create_success_response(&format!(
                        "Worker {} already active for stage {} in project {}",
                        worker.worker_id, stage, project_id
                    )));
                }
            }
        }

        // Generate unique worker ID
        let worker_id = format!("{}-{}", stage, &Uuid::new_v4().to_string()[..8]);

        let spawn_request = SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: project_id.clone(),
            worker_type: stage.clone(),
            queue_name: format!("{}-queue", stage), // Keep queue for internal implementation
        };

        match ProcessManager::spawn_worker(state, spawn_request).await {
            Ok(_worker_process) => {
                info!(
                    "Spawned worker {} for stage {} in project {}",
                    worker_id, stage, project_id
                );
                Ok(create_success_response(&format!(
                    "Successfully spawned worker {} for stage {} in project {}",
                    worker_id, stage, project_id
                )))
            }
            Err(e) => {
                warn!(
                    "Failed to spawn worker for stage {} in project {}: {}",
                    stage, project_id, e
                );
                Err(e.into())
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "spawn_worker_for_stage".to_string(),
            description: "Spawn a worker for a specific stage in a project".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stage": {
                        "type": "string",
                        "description": "Stage name (e.g., 'planning', 'design', 'coding', 'testing')"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Project identifier"
                    }
                },
                "required": ["stage", "project_id"]
            }),
        }
    }
}
