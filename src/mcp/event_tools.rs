use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{
    tools::{extract_optional_param, extract_param, ToolHandler},
    types::{CallToolResponse, Tool, ToolContent},
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

        // Most-recent-first to match "recent" semantics (unless fetching by specific IDs)
        let mut sorted_events = events;
        if event_ids.is_none() {
            sorted_events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        }

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

        Ok(CallToolResponse {
            content: vec![ToolContent {
                content_type: "application/json".to_string(),
                text: format!("Event {} resolved successfully. The event has been marked as processed and will no longer appear in unprocessed event listings.", event_id),
            }],
            is_error: Some(false),
        })
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
