use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::{
    tools::{
        create_json_error_response, create_json_success_response, extract_optional_param,
        extract_param, ToolHandler,
    },
    types::{CallToolResponse, PaginationCursor, Tool},
};
use crate::{
    database::{
        comments::{Comment, CreateCommentRequest},
        tickets::{CreateTicketRequest, Ticket, TicketState},
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
        let ticket_type: String = extract_optional_param(&Some(args.clone()), "ticket_type")?
            .unwrap_or_else(|| "task".to_string());
        let priority: String = extract_optional_param(&Some(args.clone()), "priority")?
            .unwrap_or_else(|| "medium".to_string());
        let initial_stage: String = extract_optional_param(&Some(args.clone()), "initial_stage")?
            .unwrap_or_else(|| "planning".to_string());

        // New DAG-related parameters
        let parent_ticket_id: Option<String> =
            extract_optional_param(&Some(args.clone()), "parent_ticket_id")?;
        let execution_plan_input: Option<Vec<String>> =
            extract_optional_param(&Some(args.clone()), "execution_plan")?;
        let created_by_worker_id: Option<String> =
            extract_optional_param(&Some(args.clone()), "created_by_worker_id")?;

        // Validate initial_stage only if no execution_plan is supplied
        if execution_plan_input.is_none() {
            if let Err(e) = crate::validation::PipelineValidator::validate_initial_stage(
                &state.db,
                &project_id,
                &initial_stage,
            )
            .await
            {
                return Ok(create_json_error_response(&e.to_string()));
            }
        }

        info!("Creating ticket: {} in project {}", title, project_id);

        // Use provided execution plan or default to single stage
        let execution_plan = execution_plan_input.unwrap_or_else(|| vec![initial_stage.clone()]);
        let first_stage = execution_plan.first().cloned().ok_or_else(|| {
            crate::error::AppError::BadRequest("Execution plan is empty".to_string())
        })?;

        // Validate all stages in execution plan exist as worker types
        if let Err(e) = crate::validation::PipelineValidator::validate_pipeline_stages(
            &state.db,
            &project_id,
            &execution_plan,
            "Ticket creation",
        )
        .await
        {
            return Ok(create_json_error_response(&e.to_string()));
        }

        // Get project to access project_prefix for human-friendly ticket ID
        let project =
            match crate::database::projects::Project::get_by_name(&state.db, &project_id).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    return Ok(create_json_error_response(&format!(
                        "Project '{}' not found",
                        project_id
                    )))
                }
                Err(e) => {
                    return Ok(create_json_error_response(&format!(
                        "Failed to get project: {}",
                        e
                    )))
                }
            };

        // Determine subsystem from execution plan for ticket ID generation
        let subsystem = crate::workers::ticket_id::infer_subsystem_from_stages(&execution_plan);

        // Generate human-friendly ticket ID
        let ticket_id = match crate::workers::ticket_id::generate_ticket_id(
            &state.db,
            &project.project_prefix,
            &subsystem,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                return Ok(create_json_error_response(&format!(
                    "Failed to generate ticket ID: {}",
                    e
                )))
            }
        };

        let req = CreateTicketRequest {
            ticket_id: ticket_id.clone(),
            project_id: project_id.clone(),
            title: title.clone(),
            description: description.clone(),
            execution_plan,
            parent_ticket_id,
            ticket_type: Some(ticket_type),
            dependency_status: None, // Will default to 'ready' in database
            created_by_worker_id,
            priority: Some(priority),
        };

        let ticket = match Ticket::create(&state.db, req).await {
            Ok(t) => t,
            Err(e) => {
                return Ok(create_json_error_response(&format!(
                    "Failed to create ticket: {}",
                    e
                )))
            }
        };

        // Emit ticket_created event
        if let Err(e) = state
            .event_emitter()
            .emit_ticket_created(
                &ticket.ticket_id,
                &ticket.project_id,
                &ticket.title,
                &ticket.current_stage,
            )
            .await
        {
            warn!("Failed to emit ticket_created event: {}", e);
        }

        // Automatically submit the ticket to the first stage queue
        match state
            .queue_manager
            .submit_task(&project_id, &first_stage, &ticket_id)
            .await
        {
            Ok(task_id) => {
                info!(
                    "Successfully submitted ticket {} to {}-queue as task {}",
                    ticket_id, first_stage, task_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to submit ticket {} to {}-queue: {}",
                    ticket_id, first_stage, e
                );
            }
        }

        Ok(create_json_success_response(json!({
            "message": format!("Created ticket '{}'", title),
            "ticket_id": ticket.ticket_id,
            "project_id": ticket.project_id,
            "current_stage": ticket.current_stage
        })))
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
                    },
                    "parent_ticket_id": {
                        "type": "string",
                        "description": "Optional parent ticket ID for creating subtasks"
                    },
                    "execution_plan": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Complete execution plan (array of stage names). If not provided, defaults to single initial_stage. All stages must exist as worker types."
                    },
                    "created_by_worker_id": {
                        "type": "string",
                        "description": "ID of the worker that created this ticket (for planner-created tickets)"
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

        let ticket = Ticket::get_by_id(&state.db, &ticket_id)
            .await
            .map_err(|e| {
                warn!("Failed to get ticket {}: {}", ticket_id, e);
                e
            })?;

        match ticket {
            Some(ticket_with_comments) => Ok(create_json_success_response(json!({
                "ticket": ticket_with_comments.ticket,
                "comments": ticket_with_comments.comments
            }))),
            None => Ok(create_json_error_response(&format!(
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

        // Parse pagination parameters
        let cursor_str: Option<String> = extract_optional_param(&Some(args.clone()), "cursor")?;
        let cursor = PaginationCursor::from_cursor_string(cursor_str)
            .map_err(crate::error::AppError::BadRequest)?;

        // Get all tickets first
        let all_tickets =
            Ticket::list_by_project(&state.db, project_id.as_deref(), status.as_deref())
                .await
                .map_err(|e| {
                    warn!(
                        "Failed to list tickets (project: {:?}, status: {:?}): {}",
                        project_id, status, e
                    );
                    e
                })?;

        // Apply pagination using helper
        let pagination_result = cursor.paginate(all_tickets);

        // Create response with pagination info
        let response_data = json!({
            "tickets": pagination_result.items,
            "pagination": {
                "total": pagination_result.total,
                "has_more": pagination_result.has_more,
                "next_cursor": pagination_result.next_cursor
            }
        });

        Ok(create_json_success_response(response_data))
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
                        "description": "Optional status filter (open, closed)",
                        "enum": ["open", "closed"]
                    },
                    "cursor": {
                        "type": "string",
                        "description": "Optional cursor for pagination"
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

        let comment = Comment::create_from_request(&state.db, req)
            .await
            .map_err(|e| {
                warn!("Failed to create comment for ticket {}: {}", ticket_id, e);
                e
            })?;

        // Emit ticket_updated event for comment added
        if let Err(e) = state
            .event_emitter()
            .emit_ticket_updated(
                &ticket_id,
                "", // project_id will be looked up by the emitter if needed
                "comment_added",
                None,
                Some(&format!("Comment added: {}", comment.id)),
            )
            .await
        {
            warn!("Failed to emit ticket_updated event: {}", e);
        }

        Ok(create_json_success_response(json!({
            "message": format!("Added comment to ticket {}", ticket_id),
            "ticket_id": ticket_id,
            "comment_id": comment.id
        })))
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
            "Closing ticket {} with resolution: {} (with dependency cascade)",
            ticket_id, resolution
        );

        // Use the unified completion function to close ticket and trigger dependency cascade
        match state
            .queue_manager
            .complete_ticket_with_cascade(
                &ticket_id,
                &resolution,
                &format!(
                    "Ticket closed by coordinator with resolution: {}",
                    resolution
                ),
            )
            .await
        {
            Ok(()) => Ok(create_json_success_response(json!({
                "message": format!("Closed ticket {} with resolution: {} and processed dependencies", ticket_id, resolution),
                "ticket_id": ticket_id,
                "resolution": resolution
            }))),
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(create_json_error_response(&format!(
                        "Ticket {} not found",
                        ticket_id
                    )))
                } else {
                    Err(e.into())
                }
            }
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
        let ticket = Ticket::get_by_id(&state.db, &ticket_id)
            .await
            .map_err(|e| {
                warn!("Failed to get ticket {} for resume: {}", ticket_id, e);
                e
            })?;

        let ticket_data = match ticket {
            Some(t) => t.ticket,
            None => {
                return Ok(create_json_error_response(&format!(
                    "Ticket {} not found",
                    ticket_id
                )));
            }
        };

        // Determine stage to use (provided or current)
        let target_stage = stage.unwrap_or(ticket_data.current_stage.clone());

        // Validate that the target stage worker type exists for this project
        if let Err(e) = crate::validation::PipelineValidator::validate_resume_stage(
            &state.db,
            &ticket_data.project_id,
            &target_stage,
        )
        .await
        {
            return Ok(create_json_error_response(&e.to_string()));
        }

        // Determine state to use (provided or Open)
        let target_state_enum = if let Some(state_str) = state_param {
            match state_str.parse::<TicketState>() {
                Ok(state) => state,
                Err(_) => {
                    return Ok(create_json_error_response(&format!(
                        "Invalid state '{}'. Valid states are: open, closed",
                        state_str
                    )))
                }
            }
        } else {
            TicketState::Open
        };
        let target_state = target_state_enum.to_string();

        // Update ticket stage if different
        if target_stage != ticket_data.current_stage {
            info!(
                "Updating ticket {} stage from {} to {}",
                ticket_id, ticket_data.current_stage, target_stage
            );
            Ticket::update_stage(&state.db, &ticket_id, &target_stage)
                .await
                .map_err(|e| {
                    warn!("Failed to update stage for ticket {}: {}", ticket_id, e);
                    e
                })?;
        }

        // Update ticket state if different
        if target_state != ticket_data.state {
            info!(
                "Updating ticket {} state from {} to {}",
                ticket_id, ticket_data.state, target_state
            );
            Ticket::update_state(&state.db, &ticket_id, &target_state)
                .await
                .map_err(|e| {
                    warn!("Failed to update state for ticket {}: {}", ticket_id, e);
                    e
                })?;
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
            .await
            .map_err(|e| {
                warn!(
                    "Failed to release worker claim for ticket {}: {}",
                    ticket_id, e
                );
                e
            })?;
        }

        // If state is Open, submit to queue for processing
        if matches!(target_state_enum, TicketState::Open) {
            match state
                .queue_manager
                .submit_task(&ticket_data.project_id, &target_stage, &ticket_id)
                .await
            {
                Ok(task_id) => {
                    info!(
                        "Successfully submitted ticket {} to {}-queue as task {}",
                        ticket_id, target_stage, task_id
                    );

                    Ok(create_json_success_response(json!({
                        "message": format!("Resumed processing for ticket {} at stage '{}' with state '{}' and submitted to queue as task {}", ticket_id, target_stage, target_state, task_id),
                        "ticket_id": ticket_id,
                        "target_stage": target_stage,
                        "target_state": target_state,
                        "task_id": task_id
                    })))
                }
                Err(e) => {
                    warn!(
                        "Failed to submit ticket {} to {}-queue: {}",
                        ticket_id, target_stage, e
                    );

                    Ok(create_json_success_response(json!({
                        "message": format!("Resumed ticket {} at stage '{}' with state '{}' but failed to submit to queue: {}", ticket_id, target_stage, target_state, e),
                        "ticket_id": ticket_id,
                        "target_stage": target_stage,
                        "target_state": target_state,
                        "queue_error": e.to_string()
                    })))
                }
            }
        } else {
            Ok(create_json_success_response(json!({
                "message": format!("Resumed ticket {} at stage '{}' with state '{}' (not submitted to queue due to non-open state)", ticket_id, target_stage, target_state),
                "ticket_id": ticket_id,
                "target_stage": target_stage,
                "target_state": target_state,
                "submitted_to_queue": false
            })))
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
                        "enum": TicketState::all_strings()
                    }
                },
                "required": ["ticket_id"]
            }),
        }
    }
}
