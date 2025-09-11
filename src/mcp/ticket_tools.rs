use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;
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

        info!("Creating ticket: {} in project {}", title, project_id);

        let ticket_id = Uuid::new_v4().to_string();
        let execution_plan = vec![
            "Planning".to_string(),
            "Implementation".to_string(),
            "Testing".to_string(),
            "Review".to_string(),
        ];

        let req = CreateTicketRequest {
            ticket_id: ticket_id.clone(),
            project_id: project_id.clone(),
            title: title.clone(),
            description: description.clone(),
            execution_plan,
        };

        let ticket = Ticket::create(&state.db, req).await?;

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

        let comment = Comment::create(&state.db, req).await?;

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

        info!("Updating ticket {} to stage {}", ticket_id, stage);

        let result = Ticket::update_stage(&state.db, &ticket_id, &stage).await?;

        match result {
            Some(_) => Ok(create_success_response(&format!(
                "Updated ticket {} to stage {}",
                ticket_id, stage
            ))),
            None => Ok(create_error_response(&format!(
                "Ticket {} not found",
                ticket_id
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "complete_ticket_stage".to_string(),
            description: "Update ticket's completed stage number".to_string(),
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

        let result = Ticket::close_ticket(&state.db, &ticket_id, "Completed").await?;

        match result {
            Some(_) => Ok(create_success_response(&format!(
                "Closed ticket {} with resolution: {}",
                ticket_id, resolution
            ))),
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
