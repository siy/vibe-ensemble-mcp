use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use super::{
    tools::{
        create_error_response, create_success_response, extract_optional_param, extract_param,
        ToolHandler,
    },
    types::{CallToolResponse, Tool, ToolContent},
};
use crate::{
    database::{
        comments::{Comment, CreateCommentRequest},
        tickets::{CreateTicketRequest, Ticket},
    },
    server::AppState,
};

pub struct CreateTicketTool;

#[async_trait]
impl ToolHandler for CreateTicketTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let project_id: String = extract_param(&Some(args.clone()), "project_id")?;
        let title: String = extract_param(&Some(args.clone()), "title")?;
        let description: String =
            extract_optional_param(&Some(args.clone()), "description")?.unwrap_or_default();
        let _ticket_type: String = extract_optional_param(&Some(args.clone()), "ticket_type")?
            .unwrap_or_else(|| "task".to_string());
        let _priority: String = extract_optional_param(&Some(args.clone()), "priority")?
            .unwrap_or_else(|| "medium".to_string());
        let initial_stage: String = extract_optional_param(&Some(args.clone()), "initial_stage")?
            .unwrap_or_else(|| "planning".to_string());

        // Validate that the initial stage worker type exists for this project (including "planning")
        let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
            &state.db,
            &project_id,
            &initial_stage,
        )
        .await?;

        if worker_type_exists.is_none() {
            return Ok(crate::mcp::tools::create_error_response(&format!(
                "Worker type '{}' does not exist for project '{}'. Cannot use as initial stage. Coordinator must create this worker type first.",
                initial_stage, project_id
            )));
        }

        info!("Creating ticket: {} in project {}", title, project_id);

        let ticket_id = Uuid::new_v4().to_string();
        let execution_plan = vec![initial_stage.clone()];

        let req = CreateTicketRequest {
            ticket_id: ticket_id.clone(),
            project_id: project_id.clone(),
            title: title.clone(),
            description: description.clone(),
            execution_plan,
        };

        let ticket = Ticket::create(&state.db, req).await?;

        // Broadcast ticket_created event
        let event = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {
                "uri": format!("vibe-ensemble://tickets/{}", ticket.ticket_id),
                "event": {
                    "type": "ticket_created",
                    "ticket": {
                        "ticket_id": ticket.ticket_id,
                        "project_id": ticket.project_id,
                        "title": ticket.title,
                        "execution_plan": ticket.execution_plan,
                        "current_stage": ticket.current_stage,
                        "state": ticket.state,
                        "created_at": ticket.created_at
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            }
        });
        
        if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
            tracing::warn!("Failed to broadcast ticket_created event: {}", e);
        } else {
            tracing::debug!("Successfully broadcast ticket_created event for: {}", ticket.ticket_id);
        }

        // Automatically submit the ticket to the initial stage queue
        match state
            .queue_manager
            .submit_task(&project_id, &initial_stage, &ticket_id, &state.db)
            .await
        {
            Ok(task_id) => {
                info!(
                    "Successfully submitted ticket {} to {}-queue as task {}",
                    ticket_id, initial_stage, task_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to submit ticket {} to {}-queue: {}",
                    ticket_id, initial_stage, e
                );
            }
        }

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!("Created ticket {} with ID: {}", title, ticket.ticket_id),
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_ticket".to_string(),
            description: "Create a new ticket in a project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project identifier"
                    },
                    "title": {
                        "type": "string",
                        "description": "Ticket title"
                    },
                    "description": {
                        "type": "string",
                        "description": "Ticket description"
                    },
                    "ticket_type": {
                        "type": "string",
                        "description": "Type of ticket (task, bug, feature, etc.)",
                        "default": "task"
                    },
                    "priority": {
                        "type": "string",
                        "description": "Priority level (low, medium, high, critical)",
                        "default": "medium"
                    },
                    "initial_stage": {
                        "type": "string",
                        "description": "Initial stage for ticket processing (must be a valid worker type)",
                        "default": "planning"
                    }
                },
                "required": ["project_id", "title"]
            }),
        }
    }
}

pub struct GetTicketTool;

#[async_trait]
impl ToolHandler for GetTicketTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;

        let ticket = Ticket::get_by_id(&state.db, &ticket_id).await?;

        match ticket {
            Some(ticket) => {
                let comments = Comment::get_by_ticket_id(&state.db, &ticket_id).await?;

                Ok(CallToolResponse {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: serde_json::to_string_pretty(&json!({
                            "ticket": ticket,
                            "comments": comments
                        }))?,
                    }],
                    is_error: Some(false),
                })
            }
            None => Ok(create_error_response(&format!(
                "Ticket {} not found",
                ticket_id
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_ticket".to_string(),
            description: "Get ticket details including comments and history".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    }
                },
                "required": ["ticket_id"]
            }),
        }
    }
}

pub struct ListTicketsTool;

#[async_trait]
impl ToolHandler for ListTicketsTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments.unwrap_or_default();

        let project_id: Option<String> = extract_optional_param(&Some(args.clone()), "project_id")?;
        let status: Option<String> = extract_optional_param(&Some(args.clone()), "status")?;

        let tickets =
            Ticket::list_by_project(&state.db, project_id.as_deref(), status.as_deref()).await?;

        let filtered_tickets = tickets;

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: serde_json::to_string_pretty(&filtered_tickets)?,
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_tickets".to_string(),
            description: "List tickets, optionally filtered by project or status".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project filter"
                    },
                    "status": {
                        "type": "string",
                        "description": "Optional status filter (open, in_progress, completed, closed)"
                    }
                },
                "required": []
            }),
        }
    }
}

pub struct AddTicketCommentTool;

#[async_trait]
impl ToolHandler for AddTicketCommentTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let worker_type: String = extract_param(&Some(args.clone()), "worker_type")?;
        let worker_id: String = extract_param(&Some(args.clone()), "worker_id")?;
        let stage_number: i32 = extract_param(&Some(args.clone()), "stage_number")?;
        let content: String = extract_param(&Some(args.clone()), "content")?;

        info!(
            "Adding comment to ticket {} by worker {}",
            ticket_id, worker_id
        );

        let req = CreateCommentRequest {
            ticket_id: ticket_id.clone(),
            worker_type,
            worker_id,
            stage_number,
            content: content.clone(),
        };

        let comment = Comment::create_from_request(&state.db, req).await?;

        // Broadcast ticket_comment_added event
        let event = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {
                "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
                "event": {
                    "type": "ticket_comment_added",
                    "comment": {
                        "id": comment.id,
                        "ticket_id": comment.ticket_id,
                        "worker_type": comment.worker_type,
                        "worker_id": comment.worker_id,
                        "stage_number": comment.stage_number,
                        "content": comment.content,
                        "created_at": comment.created_at
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            }
        });
        
        if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
            tracing::warn!("Failed to broadcast ticket_comment_added event: {}", e);
        } else {
            tracing::debug!("Successfully broadcast ticket_comment_added event for: {}", ticket_id);
        }

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!("Added comment to ticket {}: {}", ticket_id, comment.id),
            }],
            is_error: Some(false),
        })
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "add_ticket_comment".to_string(),
            description: "Add a worker report comment to a ticket".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Type of worker adding the comment"
                    },
                    "worker_id": {
                        "type": "string",
                        "description": "Worker identifier"
                    },
                    "stage_number": {
                        "type": "integer",
                        "description": "Stage number this comment relates to"
                    },
                    "content": {
                        "type": "string",
                        "description": "Comment content"
                    }
                },
                "required": ["ticket_id", "worker_type", "worker_id", "stage_number", "content"]
            }),
        }
    }
}

pub struct UpdateTicketStageTool;

#[async_trait]
impl ToolHandler for UpdateTicketStageTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let stage: String = extract_param(&Some(args.clone()), "stage")?;

        // Get the ticket to find the project_id
        let ticket_data = match Ticket::get_by_id(&state.db, &ticket_id).await? {
            Some(t) => t.ticket,
            None => {
                return Ok(create_error_response(&format!(
                    "Ticket {} not found",
                    ticket_id
                )));
            }
        };

        // Validate that the stage worker type exists for this project (unless it's "planning")
        if stage != "planning" {
            let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
                &state.db,
                &ticket_data.project_id,
                &stage,
            )
            .await?;

            if worker_type_exists.is_none() {
                return Ok(create_error_response(&format!(
                    "Worker type '{}' does not exist for project '{}'. Cannot update ticket to this stage.",
                    stage, ticket_data.project_id
                )));
            }
        }

        info!("Updating ticket {} to stage {}", ticket_id, stage);

        let result = Ticket::update_stage(&state.db, &ticket_id, &stage).await?;

        match result {
            Some(updated_ticket) => {
                // Broadcast ticket_stage_updated event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
                        "event": {
                            "type": "ticket_stage_updated",
                            "ticket_id": ticket_id,
                            "new_stage": stage,
                            "ticket": {
                                "ticket_id": updated_ticket.ticket_id,
                                "project_id": updated_ticket.project_id,
                                "title": updated_ticket.title,
                                "current_stage": updated_ticket.current_stage,
                                "state": updated_ticket.state,
                                "updated_at": updated_ticket.updated_at
                            },
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast ticket_stage_updated event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast ticket_stage_updated event for: {}", ticket_id);
                }

                Ok(create_success_response(&format!(
                    "Updated ticket {} to stage {}",
                    ticket_id, stage
                )))
            }
            None => Ok(create_error_response(&format!(
                "Ticket {} not found",
                ticket_id
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "update_ticket_stage".to_string(),
            description: "Update ticket to a specific stage".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    },
                    "stage": {
                        "type": "string",
                        "description": "Stage name to mark as completed"
                    }
                },
                "required": ["ticket_id", "stage"]
            }),
        }
    }
}

pub struct ClaimTicketTool;

#[async_trait]
impl ToolHandler for ClaimTicketTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let worker_id: String = extract_param(&Some(args.clone()), "worker_id")?;

        info!("Worker {} claiming ticket {}", worker_id, ticket_id);

        // Try to claim the ticket atomically - only if it's not already claimed
        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2 AND (processing_worker_id IS NULL OR processing_worker_id = '')
            "#,
        )
        .bind(&worker_id)
        .bind(&ticket_id)
        .execute(&state.db)
        .await?;

        if result.rows_affected() > 0 {
            // Broadcast ticket_claimed event
            let event = json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": {
                    "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
                    "event": {
                        "type": "ticket_claimed",
                        "ticket_id": ticket_id,
                        "worker_id": worker_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                }
            });
            
            if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                tracing::warn!("Failed to broadcast ticket_claimed event: {}", e);
            } else {
                tracing::debug!("Successfully broadcast ticket_claimed event for: {}", ticket_id);
            }

            Ok(create_success_response(&format!(
                "Successfully claimed ticket {} for worker {}",
                ticket_id, worker_id
            )))
        } else {
            Ok(create_error_response(&format!(
                "Ticket {} is already being processed by another worker",
                ticket_id
            )))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "claim_ticket".to_string(),
            description:
                "Claim a ticket for processing to prevent other workers from picking it up"
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    },
                    "worker_id": {
                        "type": "string",
                        "description": "Worker identifier claiming the ticket"
                    }
                },
                "required": ["ticket_id", "worker_id"]
            }),
        }
    }
}

pub struct ReleaseTicketTool;

#[async_trait]
impl ToolHandler for ReleaseTicketTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let worker_id: String = extract_param(&Some(args.clone()), "worker_id")?;

        info!("Worker {} releasing ticket {}", worker_id, ticket_id);

        // Release the ticket only if claimed by this specific worker
        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND processing_worker_id = ?2
            "#,
        )
        .bind(&ticket_id)
        .bind(&worker_id)
        .execute(&state.db)
        .await?;

        if result.rows_affected() > 0 {
            // Broadcast ticket_released event
            let event = json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": {
                    "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
                    "event": {
                        "type": "ticket_released",
                        "ticket_id": ticket_id,
                        "worker_id": worker_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                }
            });
            
            if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                tracing::warn!("Failed to broadcast ticket_released event: {}", e);
            } else {
                tracing::debug!("Successfully broadcast ticket_released event for: {}", ticket_id);
            }

            Ok(create_success_response(&format!(
                "Successfully released ticket {} from worker {}",
                ticket_id, worker_id
            )))
        } else {
            Ok(create_error_response(&format!(
                "Ticket {} was not claimed by worker {} or doesn't exist",
                ticket_id, worker_id
            )))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "release_ticket".to_string(),
            description: "Release a claimed ticket so other workers can process it".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    },
                    "worker_id": {
                        "type": "string",
                        "description": "Worker identifier releasing the ticket"
                    }
                },
                "required": ["ticket_id", "worker_id"]
            }),
        }
    }
}

pub struct CloseTicketTool;

#[async_trait]
impl ToolHandler for CloseTicketTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let resolution: String = extract_optional_param(&Some(args.clone()), "resolution")?
            .unwrap_or_else(|| "completed".to_string());

        info!(
            "Closing ticket {} with resolution: {}",
            ticket_id, resolution
        );

        let result = Ticket::close_ticket(&state.db, &ticket_id, &resolution).await?;

        match result {
            Some(closed_ticket) => {
                // Broadcast ticket_closed event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
                        "event": {
                            "type": "ticket_closed",
                            "ticket_id": ticket_id,
                            "resolution": resolution,
                            "ticket": {
                                "ticket_id": closed_ticket.ticket_id,
                                "project_id": closed_ticket.project_id,
                                "title": closed_ticket.title,
                                "state": closed_ticket.state,
                                "closed_at": closed_ticket.closed_at
                            },
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast ticket_closed event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast ticket_closed event for: {}", ticket_id);
                }

                Ok(create_success_response(&format!(
                    "Closed ticket {} with resolution: {}",
                    ticket_id, resolution
                )))
            }
            None => Ok(create_error_response(&format!(
                "Ticket {} not found",
                ticket_id
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "close_ticket".to_string(),
            description: "Close a ticket with optional resolution note".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier"
                    },
                    "resolution": {
                        "type": "string",
                        "description": "Resolution note",
                        "default": "completed"
                    }
                },
                "required": ["ticket_id"]
            }),
        }
    }
}

pub struct ResumeTicketProcessingTool;

#[async_trait]
impl ToolHandler for ResumeTicketProcessingTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let ticket_id: String = extract_param(&Some(args.clone()), "ticket_id")?;
        let stage: Option<String> = extract_optional_param(&Some(args.clone()), "stage")?;
        let state_param: Option<String> = extract_optional_param(&Some(args.clone()), "state")?;

        info!("Resuming processing for ticket {}", ticket_id);

        // First get the current ticket
        let ticket = Ticket::get_by_id(&state.db, &ticket_id).await?;

        let ticket_data = match ticket {
            Some(t) => t.ticket,
            None => {
                return Ok(create_error_response(&format!(
                    "Ticket {} not found",
                    ticket_id
                )));
            }
        };

        // Determine stage to use (provided or current)
        let target_stage = stage.unwrap_or(ticket_data.current_stage.clone());

        // Validate that the target stage worker type exists for this project (unless it's "planning")
        if target_stage != "planning" {
            let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
                &state.db,
                &ticket_data.project_id,
                &target_stage,
            )
            .await?;

            if worker_type_exists.is_none() {
                return Ok(create_error_response(&format!(
                    "Worker type '{}' does not exist for project '{}'. Cannot resume ticket with this stage.",
                    target_stage, ticket_data.project_id
                )));
            }
        }

        // Determine state to use (provided or "open")
        let target_state = state_param.unwrap_or_else(|| "open".to_string());

        // Update ticket stage if different
        if target_stage != ticket_data.current_stage {
            info!(
                "Updating ticket {} stage from {} to {}",
                ticket_id, ticket_data.current_stage, target_stage
            );
            Ticket::update_stage(&state.db, &ticket_id, &target_stage).await?;
        }

        // Update ticket state if different
        if target_state != ticket_data.state {
            info!(
                "Updating ticket {} state from {} to {}",
                ticket_id, ticket_data.state, target_state
            );
            Ticket::update_state(&state.db, &ticket_id, &target_state).await?;
        }

        // Release any worker claim to allow fresh processing
        if ticket_data.processing_worker_id.is_some() {
            info!("Releasing worker claim on ticket {}", ticket_id);
            sqlx::query(
                r#"
                UPDATE tickets 
                SET processing_worker_id = NULL, updated_at = datetime('now')
                WHERE ticket_id = ?1
                "#,
            )
            .bind(&ticket_id)
            .execute(&state.db)
            .await?;
        }

        // If state is "open", submit to queue for processing
        if target_state == "open" {
            match state
                .queue_manager
                .submit_task(
                    &ticket_data.project_id,
                    &target_stage,
                    &ticket_id,
                    &state.db,
                )
                .await
            {
                Ok(task_id) => {
                    info!(
                        "Successfully submitted ticket {} to {}-queue as task {}",
                        ticket_id, target_stage, task_id
                    );

                    Ok(create_success_response(&format!(
                        "Resumed processing for ticket {} at stage '{}' with state '{}' and submitted to queue as task {}",
                        ticket_id, target_stage, target_state, task_id
                    )))
                }
                Err(e) => {
                    warn!(
                        "Failed to submit ticket {} to {}-queue: {}",
                        ticket_id, target_stage, e
                    );

                    Ok(create_success_response(&format!(
                        "Resumed ticket {} at stage '{}' with state '{}' but failed to submit to queue: {}",
                        ticket_id, target_stage, target_state, e
                    )))
                }
            }
        } else {
            Ok(create_success_response(&format!(
                "Resumed ticket {} at stage '{}' with state '{}' (not submitted to queue due to non-open state)",
                ticket_id, target_stage, target_state
            )))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "resume_ticket_processing".to_string(),
            description: "Resume processing of a ticket that was put on hold or stopped, optionally changing stage and state".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket identifier to resume"
                    },
                    "stage": {
                        "type": "string",
                        "description": "Optional stage to resume from (uses current stage if not specified)"
                    },
                    "state": {
                        "type": "string",
                        "description": "Optional ticket state (open/closed/on_hold, defaults to 'open')",
                        "enum": ["open", "closed", "on_hold"]
                    }
                },
                "required": ["ticket_id"]
            }),
        }
    }
}
