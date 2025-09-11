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

#[derive(Debug, Clone, Serialize)]
pub struct TicketWithComments {
    pub ticket: Ticket,
    pub comments: Vec<crate::database::comments::Comment>,
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
            conditions.push(format!("project_id = ?1"));
        }
        
        if status_filter.is_some() {
            let condition = if status_filter == Some("open") {
                format!("closed_at IS NULL")
            } else if status_filter == Some("closed") {
                format!("closed_at IS NOT NULL")
            } else {
                // Invalid status filter, ignore
                String::new()
            };
            if !condition.is_empty() {
                conditions.push(condition);
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

        if let Some(ref _ticket) = ticket {
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