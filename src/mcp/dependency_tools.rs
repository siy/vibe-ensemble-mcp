use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::{
    tools::{
        create_json_error_response, create_json_success_response, extract_optional_param, extract_param,
        ToolHandler,
    },
    types::{CallToolResponse, PaginationCursor, Tool, ToolContent},
};
use crate::{database::dag::TicketDependency, server::AppState};

pub struct AddTicketDependencyTool;

#[async_trait]
impl ToolHandler for AddTicketDependencyTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let parent_ticket_id: String = extract_param(&Some(args.clone()), "parent_ticket_id")?;
        let child_ticket_id: String = extract_param(&Some(args.clone()), "child_ticket_id")?;
        let dependency_type: String =
            extract_optional_param(&Some(args.clone()), "dependency_type")?
                .unwrap_or_else(|| "blocks".to_string());

        info!(
            "Adding dependency: {} -> {} (type: {})",
            parent_ticket_id, child_ticket_id, dependency_type
        );

        match TicketDependency::create(
            &state.db,
            &parent_ticket_id,
            &child_ticket_id,
            &dependency_type,
        )
        .await
        {
            Ok(_dependency) => {
                info!(
                    "Successfully created dependency: {} -> {}",
                    parent_ticket_id, child_ticket_id
                );

                // If this is a blocking dependency, update child ticket status to blocked
                if dependency_type == "blocks" {
                    let _ = crate::database::tickets::Ticket::update_dependency_status(
                        &state.db,
                        &child_ticket_id,
                        "blocked",
                    )
                    .await;
                }

                Ok(create_json_success_response(json!({
                    "message": format!("Successfully created {} dependency from '{}' to '{}'", dependency_type, parent_ticket_id, child_ticket_id),
                    "dependency_type": dependency_type,
                    "parent_ticket_id": parent_ticket_id,
                    "child_ticket_id": child_ticket_id
                })))
            }
            Err(e) => {
                warn!(
                    "Failed to create dependency {}->{}: {}",
                    parent_ticket_id, child_ticket_id, e
                );
                Ok(create_json_error_response(&format!(
                    "Failed to create dependency: {}",
                    e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "add_ticket_dependency".to_string(),
            description: "Create a dependency relationship between two tickets. Parent ticket must complete before child ticket can proceed (for 'blocks' type).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "parent_ticket_id": {
                        "type": "string",
                        "description": "ID of the parent ticket (the dependency)"
                    },
                    "child_ticket_id": {
                        "type": "string",
                        "description": "ID of the child ticket (depends on parent)"
                    },
                    "dependency_type": {
                        "type": "string",
                        "description": "Type of dependency",
                        "enum": ["blocks", "subtask"],
                        "default": "blocks"
                    }
                },
                "required": ["parent_ticket_id", "child_ticket_id"]
            }),
        }
    }
}

pub struct RemoveTicketDependencyTool;

#[async_trait]
impl ToolHandler for RemoveTicketDependencyTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let parent_ticket_id: String = extract_param(&Some(args.clone()), "parent_ticket_id")?;
        let child_ticket_id: String = extract_param(&Some(args.clone()), "child_ticket_id")?;

        info!(
            "Removing dependency: {} -> {}",
            parent_ticket_id, child_ticket_id
        );

        match TicketDependency::remove(&state.db, &parent_ticket_id, &child_ticket_id).await {
            Ok(_) => {
                info!(
                    "Successfully removed dependency: {} -> {}",
                    parent_ticket_id, child_ticket_id
                );

                // Check if child ticket should be unblocked
                if TicketDependency::all_dependencies_satisfied(&state.db, &child_ticket_id)
                    .await
                    .unwrap_or(false)
                {
                    let _ = crate::database::tickets::Ticket::update_dependency_status(
                        &state.db,
                        &child_ticket_id,
                        "ready",
                    )
                    .await;
                }

                Ok(create_json_success_response(json!({
                    "message": format!("Successfully removed dependency from '{}' to '{}'", parent_ticket_id, child_ticket_id),
                    "parent_ticket_id": parent_ticket_id,
                    "child_ticket_id": child_ticket_id
                })))
            }
            Err(e) => {
                warn!(
                    "Failed to remove dependency {}->{}: {}",
                    parent_ticket_id, child_ticket_id, e
                );
                Ok(create_json_error_response(&format!(
                    "Failed to remove dependency: {}",
                    e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "remove_ticket_dependency".to_string(),
            description: "Remove a dependency relationship between two tickets".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "parent_ticket_id": {
                        "type": "string",
                        "description": "ID of the parent ticket"
                    },
                    "child_ticket_id": {
                        "type": "string",
                        "description": "ID of the child ticket"
                    }
                },
                "required": ["parent_ticket_id", "child_ticket_id"]
            }),
        }
    }
}

pub struct GetDependencyGraphTool;

#[async_trait]
impl ToolHandler for GetDependencyGraphTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let project_id: String = extract_param(&Some(args.clone()), "project_id")?;
        let _depth: Option<i32> = extract_optional_param(&Some(args.clone()), "depth")?;

        info!("Building dependency graph for project: {}", project_id);

        match TicketDependency::build_project_graph(&state.db, &project_id).await {
            Ok(graph) => {
                info!(
                    "Successfully built dependency graph with {} nodes and {} edges",
                    graph.nodes.len(),
                    graph.edges.len()
                );

                Ok(CallToolResponse {
                    content: vec![ToolContent {
                        content_type: "application/json".to_string(),
                        text: serde_json::to_string_pretty(&graph)?,
                    }],
                    is_error: Some(false),
                })
            }
            Err(e) => {
                warn!(
                    "Failed to build dependency graph for project {}: {}",
                    project_id, e
                );
                Ok(create_json_error_response(&format!(
                    "Failed to build dependency graph: {}",
                    e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_dependency_graph".to_string(),
            description: "Get the complete dependency graph for a project, showing all tickets and their relationships".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project identifier"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse (optional, defaults to unlimited)",
                        "minimum": 1
                    }
                },
                "required": ["project_id"]
            }),
        }
    }
}

pub struct ListReadyTicketsTool;

#[async_trait]
impl ToolHandler for ListReadyTicketsTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let project_id: Option<String> = extract_optional_param(&Some(args.clone()), "project_id")?;

        // Parse pagination parameters
        let cursor_str: Option<String> = extract_optional_param(&Some(args.clone()), "cursor")?;
        let cursor = PaginationCursor::from_cursor_string(cursor_str)
            .map_err(crate::error::AppError::BadRequest)?;

        info!("Listing ready tickets for project: {:?}", project_id);

        match crate::database::tickets::Ticket::get_ready_tickets(&state.db, project_id.as_deref())
            .await
        {
            Ok(all_tickets) => {
                info!("Found {} ready tickets", all_tickets.len());

                // Apply pagination using helper
                let pagination_result = cursor.paginate(all_tickets);

                // Create response with pagination info
                let response_data = serde_json::json!({
                    "tickets": pagination_result.items,
                    "pagination": {
                        "total": pagination_result.total,
                        "has_more": pagination_result.has_more,
                        "next_cursor": pagination_result.next_cursor
                    }
                });

                Ok(CallToolResponse {
                    content: vec![ToolContent {
                        content_type: "application/json".to_string(),
                        text: serde_json::to_string_pretty(&response_data)?,
                    }],
                    is_error: Some(false),
                })
            }
            Err(e) => {
                warn!("Failed to list ready tickets: {}", e);
                Ok(create_json_error_response(&format!(
                    "Failed to list ready tickets: {}",
                    e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_ready_tickets".to_string(),
            description:
                "List all tickets that are ready to be processed (no blocking dependencies)"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project identifier to filter tickets"
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

pub struct ListBlockedTicketsTool;

#[async_trait]
impl ToolHandler for ListBlockedTicketsTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let project_id: Option<String> = extract_optional_param(&Some(args.clone()), "project_id")?;

        // Parse pagination parameters
        let cursor_str: Option<String> = extract_optional_param(&Some(args.clone()), "cursor")?;
        let cursor = PaginationCursor::from_cursor_string(cursor_str)
            .map_err(crate::error::AppError::BadRequest)?;

        info!("Listing blocked tickets for project: {:?}", project_id);

        match crate::database::tickets::Ticket::get_blocked_tickets(
            &state.db,
            project_id.as_deref(),
        )
        .await
        {
            Ok(all_tickets) => {
                info!("Found {} blocked tickets", all_tickets.len());

                // Apply pagination using helper
                let pagination_result = cursor.paginate(all_tickets);

                // Create response with pagination info
                let response_data = serde_json::json!({
                    "tickets": pagination_result.items,
                    "pagination": {
                        "total": pagination_result.total,
                        "has_more": pagination_result.has_more,
                        "next_cursor": pagination_result.next_cursor
                    }
                });

                Ok(CallToolResponse {
                    content: vec![ToolContent {
                        content_type: "application/json".to_string(),
                        text: serde_json::to_string_pretty(&response_data)?,
                    }],
                    is_error: Some(false),
                })
            }
            Err(e) => {
                warn!("Failed to list blocked tickets: {}", e);
                Ok(create_json_error_response(&format!(
                    "Failed to list blocked tickets: {}",
                    e
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_blocked_tickets".to_string(),
            description: "List all tickets that are blocked by dependencies".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project identifier to filter tickets"
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
