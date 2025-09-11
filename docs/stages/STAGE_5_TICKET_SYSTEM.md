# Stage 5: Ticket System

**Duration**: 2-3 hours  
**Goal**: Complete ticket workflow and event system

## Overview

This stage implements the complete ticket management system with multi-stage workflow, comment system, and event notifications. This includes ticket CRUD operations, stage progression logic, worker task assignment, and coordinator event notifications.

## Objectives

1. Implement complete ticket CRUD operations (6 tools)
2. Create comment system with atomic stage updates
3. Build event generation and management system (2 tools)
4. Implement stage progression logic and validation
5. Add task assignment to worker queues
6. Create worker-specific tools for ticket processing

## Ticket Workflow

```
┌─────────────────┐    create_ticket    ┌──────────────────┐
│   Coordinator   │────────────────────►│     Ticket       │
│                 │                     │   (Planned)      │
│                 │    queue_task       │                  │
│                 │────────────────────►│  ┌─────────────┐ │
│                 │                     │  │   Queue     │ │
│                 │                     │  │   Task      │ │
│                 │                     │  └─────────────┘ │
└─────────────────┘                     └──────────────────┘
                                                   │
                   ┌─────────────────────────────────┘
                   │
                   ▼
        ┌─────────────────┐    process_task    ┌──────────────────┐
        │     Worker      │◄──────────────────►│     Ticket       │
        │   (Stage 1)     │                    │  (Stage 1 Done)  │
        │                 │  add_comment +     │                  │
        │                 │  complete_stage    │ ┌─────────────┐ │
        │                 │───────────────────►│ │  Comment 1  │ │
        └─────────────────┘                    │ │ (Worker A)  │ │
                                               │ └─────────────┘ │
                                               └──────────────────┘
                                                        │
                        ┌───────────────────────────────┘
                        │
                        ▼                 event_generated
              ┌─────────────────┐◄──────────────────────────────────┐
              │   Coordinator   │                                   │
              │                 │    get_events                     │
              │                 │◄─────────────────                 │
              └─────────────────┘                                   │
                        │                                           │
                        │ assign_next_worker                        │
                        ▼                                           │
              ┌─────────────────┐    queue_task    ┌──────────────────┐
              │     Worker      │◄─────────────────│     Ticket       │
              │   (Stage 2)     │                  │  (Stage 2 Todo)  │
              └─────────────────┘                  └──────────────────┘
```

## Implementation

### 1. Ticket Database Operations (`src/database/tickets.rs`)

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use anyhow::Result;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Ticket {
    pub ticket_id: String,
    pub project_id: String,
    pub title: String,
    pub execution_plan: String, // JSON array
    pub last_completed_stage: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTicketRequest {
    pub ticket_id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub execution_plan: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTicketStageRequest {
    pub new_stage: String,
}

impl Ticket {
    pub async fn create(pool: &DbPool, req: CreateTicketRequest) -> Result<Ticket> {
        let mut tx = pool.begin().await?;

        // Create ticket
        let execution_plan_json = serde_json::to_string(&req.execution_plan)?;
        
        let ticket = sqlx::query_as::<_, Ticket>(r#"
            INSERT INTO tickets (ticket_id, project_id, title, execution_plan, last_completed_stage)
            VALUES (?1, ?2, ?3, ?4, 'Planned')
            RETURNING ticket_id, project_id, title, execution_plan, last_completed_stage, 
                     created_at, updated_at, closed_at
        "#)
        .bind(&req.ticket_id)
        .bind(&req.project_id)
        .bind(&req.title)
        .bind(&execution_plan_json)
        .fetch_one(&mut *tx)
        .await?;

        // Add initial comment with description
        sqlx::query(r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, 'coordinator', 'coordinator', 0, ?2)
        "#)
        .bind(&req.ticket_id)
        .bind(&req.description)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ticket)
    }

    pub async fn get_by_id(pool: &DbPool, ticket_id: &str) -> Result<Option<TicketWithComments>> {
        let ticket = sqlx::query_as::<_, Ticket>(r#"
            SELECT ticket_id, project_id, title, execution_plan, last_completed_stage,
                   created_at, updated_at, closed_at
            FROM tickets
            WHERE ticket_id = ?1
        "#)
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        if let Some(ticket) = ticket {
            let comments = crate::database::comments::Comment::get_by_ticket_id(pool, ticket_id).await?;
            Ok(Some(TicketWithComments { ticket, comments }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_by_project(
        pool: &DbPool, 
        project_id: Option<&str>,
        status_filter: Option<&str>
    ) -> Result<Vec<Ticket>> {
        let mut query = String::from(r#"
            SELECT ticket_id, project_id, title, execution_plan, last_completed_stage,
                   created_at, updated_at, closed_at
            FROM tickets
        "#);

        let mut conditions = Vec::new();
        if project_id.is_some() {
            conditions.push("project_id = ?1");
        }
        if status_filter.is_some() {
            if project_id.is_some() {
                conditions.push("(closed_at IS NULL) = ?2");
            } else {
                conditions.push("(closed_at IS NULL) = ?1");
            }
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }
        query.push_str(" ORDER BY created_at DESC");

        let mut query_builder = sqlx::query_as::<_, Ticket>(&query);
        
        if let Some(pid) = project_id {
            query_builder = query_builder.bind(pid);
        }
        if let Some(status) = status_filter {
            let is_open = status == "open";
            query_builder = query_builder.bind(is_open);
        }

        let tickets = query_builder.fetch_all(pool).await?;
        Ok(tickets)
    }

    pub async fn update_stage(pool: &DbPool, ticket_id: &str, new_stage: &str) -> Result<Option<Ticket>> {
        let ticket = sqlx::query_as::<_, Ticket>(r#"
            UPDATE tickets 
            SET last_completed_stage = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, last_completed_stage,
                     created_at, updated_at, closed_at
        "#)
        .bind(new_stage)
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        Ok(ticket)
    }

    pub async fn close_ticket(pool: &DbPool, ticket_id: &str, status: &str) -> Result<Option<Ticket>> {
        let mut tx = pool.begin().await?;

        // Update ticket status
        let ticket = sqlx::query_as::<_, Ticket>(r#"
            UPDATE tickets 
            SET last_completed_stage = ?1, updated_at = datetime('now'), closed_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, last_completed_stage,
                     created_at, updated_at, closed_at
        "#)
        .bind(status)
        .bind(ticket_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(ref ticket) = ticket {
            // Add closing comment
            let closing_message = match status {
                "Completed" => "Ticket completed successfully by coordinator.",
                "Stopped" => "Ticket stopped by coordinator due to issues or cancellation.",
                _ => "Ticket closed by coordinator.",
            };

            sqlx::query(r#"
                INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
                VALUES (?1, 'coordinator', 'coordinator', 999, ?2)
            "#)
            .bind(ticket_id)
            .bind(closing_message)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(ticket)
    }

    pub fn get_execution_plan(&self) -> Result<Vec<String>> {
        Ok(serde_json::from_str(&self.execution_plan)?)
    }

    pub fn get_next_stage(&self) -> Result<Option<String>> {
        let plan = self.get_execution_plan()?;
        
        if self.last_completed_stage == "Planned" {
            return Ok(plan.first().cloned());
        }

        // Find current stage index and return next
        for (i, stage) in plan.iter().enumerate() {
            if stage == &self.last_completed_stage {
                return Ok(plan.get(i + 1).cloned());
            }
        }

        Ok(None)
    }

    pub fn is_completed(&self) -> bool {
        self.closed_at.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketWithComments {
    pub ticket: Ticket,
    pub comments: Vec<crate::database::comments::Comment>,
}
```

### 2. Comment Database Operations (`src/database/comments.rs`)

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use anyhow::Result;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Comment {
    pub id: i64,
    pub ticket_id: String,
    pub worker_type: Option<String>,
    pub worker_id: Option<String>,
    pub stage_number: Option<i32>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub ticket_id: String,
    pub worker_type: String,
    pub worker_id: String,
    pub stage_number: i32,
    pub content: String,
}

impl Comment {
    pub async fn create(pool: &DbPool, req: CreateCommentRequest) -> Result<Comment> {
        let comment = sqlx::query_as::<_, Comment>(r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, ticket_id, worker_type, worker_id, stage_number, content, created_at
        "#)
        .bind(&req.ticket_id)
        .bind(&req.worker_type)
        .bind(&req.worker_id)
        .bind(req.stage_number)
        .bind(&req.content)
        .fetch_one(pool)
        .await?;

        Ok(comment)
    }

    pub async fn get_by_ticket_id(pool: &DbPool, ticket_id: &str) -> Result<Vec<Comment>> {
        let comments = sqlx::query_as::<_, Comment>(r#"
            SELECT id, ticket_id, worker_type, worker_id, stage_number, content, created_at
            FROM comments
            WHERE ticket_id = ?1
            ORDER BY created_at ASC
        "#)
        .bind(ticket_id)
        .fetch_all(pool)
        .await?;

        Ok(comments)
    }

    pub async fn add_with_stage_update(
        pool: &DbPool,
        req: CreateCommentRequest,
        new_stage: &str,
    ) -> Result<(Comment, bool)> {
        let mut tx = pool.begin().await?;

        // Add comment
        let comment = sqlx::query_as::<_, Comment>(r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, ticket_id, worker_type, worker_id, stage_number, content, created_at
        "#)
        .bind(&req.ticket_id)
        .bind(&req.worker_type)
        .bind(&req.worker_id)
        .bind(req.stage_number)
        .bind(&req.content)
        .fetch_one(&mut *tx)
        .await?;

        // Update ticket stage
        let updated_rows = sqlx::query(r#"
            UPDATE tickets 
            SET last_completed_stage = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
        "#)
        .bind(new_stage)
        .bind(&req.ticket_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok((comment, updated_rows.rows_affected() > 0))
    }
}
```

### 3. Event System (`src/database/events.rs`)

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use anyhow::Result;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: i64,
    pub event_type: String,
    pub ticket_id: Option<String>,
    pub worker_id: Option<String>,
    pub stage: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
    pub processed: bool,
}

impl Event {
    pub async fn create_stage_completed(
        pool: &DbPool,
        ticket_id: &str,
        stage: &str,
        worker_id: &str,
    ) -> Result<Event> {
        let event = sqlx::query_as::<_, Event>(r#"
            INSERT INTO events (event_type, ticket_id, worker_id, stage)
            VALUES ('ticket_stage_completed', ?1, ?2, ?3)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
        "#)
        .bind(ticket_id)
        .bind(worker_id)
        .bind(stage)
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn create_worker_stopped(
        pool: &DbPool,
        worker_id: &str,
        reason: &str,
    ) -> Result<Event> {
        let event = sqlx::query_as::<_, Event>(r#"
            INSERT INTO events (event_type, worker_id, reason)
            VALUES ('worker_stopped', ?1, ?2)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
        "#)
        .bind(worker_id)
        .bind(reason)
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn get_unprocessed(pool: &DbPool) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
            FROM events
            WHERE processed = 0
            ORDER BY created_at ASC
        "#)
        .fetch_all(pool)
        .await?;

        Ok(events)
    }

    pub async fn get_all(pool: &DbPool, processed_filter: Option<bool>) -> Result<Vec<Event>> {
        let query = match processed_filter {
            Some(processed) => {
                format!(r#"
                    SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
                    FROM events
                    WHERE processed = {}
                    ORDER BY created_at DESC
                "#, if processed { 1 } else { 0 })
            }
            None => r#"
                SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
                FROM events
                ORDER BY created_at DESC
            "#.to_string(),
        };

        let events = sqlx::query_as::<_, Event>(&query)
            .fetch_all(pool)
            .await?;

        Ok(events)
    }

    pub async fn mark_processed(pool: &DbPool, event_ids: &[i64]) -> Result<u64> {
        if event_ids.is_empty() {
            return Ok(0);
        }

        let placeholders = event_ids.iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        let query = format!(r#"
            UPDATE events 
            SET processed = 1 
            WHERE id IN ({})
        "#, placeholders);

        let mut query_builder = sqlx::query(&query);
        for id in event_ids {
            query_builder = query_builder.bind(id);
        }

        let result = query_builder.execute(pool).await?;
        Ok(result.rows_affected())
    }
}
```

### 4. Ticket Management Tools (`src/mcp/ticket_tools.rs`)

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    database::{
        tickets::{Ticket, CreateTicketRequest, TicketWithComments},
        comments::{Comment, CreateCommentRequest},
        events::Event,
    },
    error::Result,
    server::AppState,
};
use super::tools::{
    ToolHandler, extract_param, extract_optional_param, create_success_response, create_error_response
};
use super::types::{CallToolResponse, Tool};

pub struct CreateTicketTool;

#[async_trait]
impl ToolHandler for CreateTicketTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let ticket_id: String = extract_param(&arguments, "ticket_id")?;
        let project_id: String = extract_param(&arguments, "project_id")?;
        let title: String = extract_param(&arguments, "title")?;
        let description: String = extract_param(&arguments, "description")?;
        let execution_plan: Vec<String> = extract_param(&arguments, "execution_plan")?;

        let request = CreateTicketRequest {
            ticket_id: ticket_id.clone(),
            project_id,
            title,
            description,
            execution_plan,
        };

        match Ticket::create(&state.db, request).await {
            Ok(ticket) => {
                let response = json!({
                    "ticket_id": ticket.ticket_id,
                    "project_id": ticket.project_id,
                    "title": ticket.title,
                    "execution_plan": ticket.get_execution_plan().unwrap_or_default(),
                    "status": ticket.last_completed_stage
                });
                Ok(create_success_response(&format!("Ticket created successfully: {}", response)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to create ticket: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_ticket".to_string(),
            description: "Create a new ticket with execution plan".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Unique ticket identifier"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "title": {
                        "type": "string",
                        "description": "Ticket title"
                    },
                    "description": {
                        "type": "string",
                        "description": "Detailed description of what should be done"
                    },
                    "execution_plan": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Ordered list of worker types for execution stages"
                    }
                },
                "required": ["ticket_id", "project_id", "title", "description", "execution_plan"]
            }),
        }
    }
}

pub struct GetTicketTool;

#[async_trait]
impl ToolHandler for GetTicketTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let ticket_id: String = extract_param(&arguments, "ticket_id")?;

        match Ticket::get_by_id(&state.db, &ticket_id).await {
            Ok(Some(ticket_with_comments)) => {
                let ticket_json = serde_json::to_string_pretty(&ticket_with_comments)?;
                Ok(create_success_response(&format!("Ticket:\n{}", ticket_json)))
            }
            Ok(None) => Ok(create_error_response(&format!("Ticket '{}' not found", ticket_id))),
            Err(e) => Ok(create_error_response(&format!("Failed to get ticket: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_ticket".to_string(),
            description: "Get ticket details with all comments".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket ID to retrieve"
                    }
                },
                "required": ["ticket_id"]
            }),
        }
    }
}

pub struct AddTicketCommentTool;

#[async_trait]
impl ToolHandler for AddTicketCommentTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let ticket_id: String = extract_param(&arguments, "ticket_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;
        let worker_id: String = extract_param(&arguments, "worker_id")?;
        let stage_number: i32 = extract_param(&arguments, "stage_number")?;
        let content: String = extract_param(&arguments, "content")?;

        let request = CreateCommentRequest {
            ticket_id: ticket_id.clone(),
            worker_type,
            worker_id,
            stage_number,
            content,
        };

        match Comment::create(&state.db, request).await {
            Ok(comment) => {
                let response = json!({
                    "comment_id": comment.id,
                    "ticket_id": comment.ticket_id,
                    "worker_type": comment.worker_type,
                    "stage_number": comment.stage_number,
                    "created_at": comment.created_at
                });
                Ok(create_success_response(&format!("Comment added successfully: {}", response)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to add comment: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "add_ticket_comment".to_string(),
            description: "Add a comment to a ticket".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket ID"
                    },
                    "worker_type": {
                        "type": "string", 
                        "description": "Type of worker adding comment"
                    },
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID adding comment"
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

// Additional tools: ListTicketsTool, UpdateTicketStageTool, CloseTicketTool
// Implementation follows similar patterns...
```

### 5. Task Assignment Tools

```rust
pub struct QueueTaskTool;

#[async_trait]
impl ToolHandler for QueueTaskTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let ticket_id: String = extract_param(&arguments, "ticket_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;
        
        // Find worker of the specified type
        let workers = crate::database::workers::Worker::list_by_type(&state.db, &worker_type).await?;
        let active_worker = workers.into_iter()
            .find(|w| w.status == "active" || w.status == "idle");

        match active_worker {
            Some(worker) => {
                // Add task to worker's queue
                match state.queue_manager.add_task(&worker.queue_name, &ticket_id).await {
                    Ok(task_id) => {
                        let response = json!({
                            "task_id": task_id,
                            "ticket_id": ticket_id,
                            "queue_name": worker.queue_name,
                            "worker_id": worker.worker_id
                        });
                        Ok(create_success_response(&format!("Task queued successfully: {}", response)))
                    }
                    Err(e) => Ok(create_error_response(&format!("Failed to queue task: {}", e))),
                }
            }
            None => Ok(create_error_response(&format!("No active worker found for type '{}'", worker_type))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "queue_task".to_string(),
            description: "Queue a ticket task for a specific worker type".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ticket_id": {
                        "type": "string",
                        "description": "Ticket ID to queue"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type to assign the task to"
                    }
                },
                "required": ["ticket_id", "worker_type"]
            }),
        }
    }
}
```

## Testing

### 1. Complete Workflow Test

```bash
# 1. Create ticket
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "name": "create_ticket",
      "arguments": {
        "ticket_id": "TICKET-001",
        "project_id": "test/project",
        "title": "Implement user authentication",
        "description": "Create secure user authentication system",
        "execution_plan": ["architect", "developer", "tester"]
      }
    }
  }'

# 2. Queue task
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "call_tool",
    "params": {
      "name": "queue_task",
      "arguments": {
        "ticket_id": "TICKET-001",
        "worker_type": "architect"
      }
    }
  }'

# 3. Check events
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "call_tool",
    "params": {
      "name": "get_events",
      "arguments": {
        "processed": false
      }
    }
  }'
```

## Validation Checklist

- [ ] Tickets can be created with execution plans
- [ ] Comments can be added to tickets
- [ ] Stage progression works correctly
- [ ] Events are generated for stage completions
- [ ] Task queuing works properly
- [ ] Worker task assignment functions
- [ ] Ticket closure process works

## Next Steps

After completing Stage 5:
1. Test complete ticket workflow end-to-end
2. Verify event system works correctly
3. Update progress in [TODO.md](../TODO.md)
4. Proceed to [Stage 6: Integration & Testing](STAGE_6_INTEGRATION_TESTING.md)