use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{
    pagination::extract_cursor,
    tools::{
        create_json_success_response, create_success_response, extract_optional_param,
        extract_param, ToolHandler,
    },
    types::{CallToolResponse, Tool},
};
use crate::{
    database::{events::Event, tickets::Ticket},
    server::AppState,
};

pub struct ListEventsTool;

#[async_trait]
impl ToolHandler for ListEventsTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments.unwrap_or_else(|| Value::Object(serde_json::Map::new()));

        let event_type: Option<String> = extract_optional_param(&Some(args.clone()), "event_type")?;
        let limit: i32 = extract_optional_param(&Some(args.clone()), "limit")?.unwrap_or(50);
        let include_processed: bool =
            extract_optional_param(&Some(args.clone()), "include_processed")?.unwrap_or(false);
        let event_ids: Option<Vec<i64>> = extract_optional_param(&Some(args.clone()), "event_ids")?;

        // Parse pagination parameters using helper
        let cursor = extract_cursor(&Some(args.clone()))?;

        let events = if let Some(ref ids) = event_ids {
            // Get specific events by IDs (ignores processed filter when using specific IDs)
            Event::get_by_ids(&state.db, ids).await?
        } else if include_processed {
            // Get all events (processed and unprocessed)
            Event::get_all(&state.db, None).await?
        } else {
            // Get only unprocessed events (default behavior)
            Event::get_unprocessed(&state.db).await?
        };

        // Events are already ordered by ID (chronological) from database
        let sorted_events = events;

        let filtered_events: Vec<_> = sorted_events
            .into_iter()
            .filter(|event| {
                // Filter by event type if specified
                if let Some(ref type_filter) = event_type {
                    &event.event_type == type_filter
                } else {
                    true
                }
            })
            .take(limit as usize)
            .collect();

        // Apply pagination using helper
        let pagination_result = cursor.paginate(filtered_events);

        // Create response with pagination info
        let response_data = serde_json::json!({
            "events": pagination_result.items,
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
            name: "list_events".to_string(),
            description: "List system events with flexible filtering options. By default shows recent unprocessed events, but can show all events or specific events by ID.".to_string(),
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
                    },
                    "include_processed": {
                        "type": "boolean",
                        "description": "Include processed events in results. When true, shows all events regardless of processed status.",
                        "default": false
                    },
                    "event_ids": {
                        "type": "array",
                        "items": {
                            "type": "integer"
                        },
                        "description": "Get specific events by their IDs. When provided, ignores include_processed filter and other filtering options."
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

pub struct ResolveEventTool;

#[async_trait]
impl ToolHandler for ResolveEventTool {
    async fn call(
        &self,
        state: &AppState,
        arguments: Option<Value>,
    ) -> crate::error::Result<CallToolResponse> {
        let args = arguments
            .ok_or_else(|| crate::error::AppError::BadRequest("Missing arguments".to_string()))?;

        let event_id: i64 = extract_param(&Some(args.clone()), "event_id")?;
        let resolution_summary: String = extract_param(&Some(args.clone()), "resolution_summary")?;

        info!(
            "Resolving event {} (summary len: {} chars)",
            event_id,
            resolution_summary.len()
        );

        Event::resolve_event(&state.db, event_id, &resolution_summary).await?;

        Ok(create_success_response(&format!("Event {} resolved successfully. The event has been marked as processed and will no longer appear in unprocessed event listings.", event_id)))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "resolve_event".to_string(),
            description: "Mark an event as resolved with a summary of investigation and actions taken. This marks the event as processed so it no longer appears in active event listings.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "event_id": {
                        "type": "integer",
                        "description": "ID of the event to resolve"
                    },
                    "resolution_summary": {
                        "type": "string",
                        "description": "Summary of the investigation and actions taken to address the event"
                    }
                },
                "required": ["event_id", "resolution_summary"]
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

        // Parse pagination parameters using helper
        let cursor = extract_cursor(&Some(args.clone()))?;

        info!("Getting tickets for stage: {}", stage);

        // Get all tickets with matching current_stage using database function
        let all_tickets = Ticket::list_open_by_stage(&state.db, &stage).await?;

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

        Ok(create_json_success_response(response_data))
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
                    },
                    "cursor": {
                        "type": "string",
                        "description": "Optional cursor for pagination"
                    }
                },
                "required": ["stage"]
            }),
        }
    }
}
