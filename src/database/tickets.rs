use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Ticket {
    pub ticket_id: String,
    pub project_id: String,
    pub title: String,
    pub execution_plan: String, // JSON array
    pub current_stage: String,
    pub state: String,
    pub priority: String,
    pub processing_worker_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    // New fields for DAG support
    pub parent_ticket_id: Option<String>,
    pub dependency_status: String,
    pub created_by_worker_id: Option<String>,
    pub ticket_type: String,
    pub rules_version: Option<i32>,
    pub patterns_version: Option<i32>,
    pub inherited_from_parent: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateTicketRequest {
    pub ticket_id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub execution_plan: Vec<String>,
    // New fields for DAG support
    pub parent_ticket_id: Option<String>,
    pub ticket_type: Option<String>,
    pub dependency_status: Option<String>,
    pub created_by_worker_id: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct TicketWithProjectInfo {
    pub ticket: Ticket,
    pub project_rules: Option<String>,
    pub project_patterns: Option<String>,
}

impl Ticket {
    pub async fn create(pool: &DbPool, req: CreateTicketRequest) -> Result<Ticket> {
        let mut tx = pool.begin().await?;

        // Create ticket
        let execution_plan_json = serde_json::to_string(&req.execution_plan)?;

        // Get project info for rules/patterns versioning
        let project = crate::database::projects::Project::get_by_name(pool, &req.project_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", req.project_id))?;

        // Determine initial stage from execution plan
        let initial_stage = if req.execution_plan.is_empty() {
            "planning".to_string()
        } else {
            req.execution_plan[0].clone()
        };

        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            INSERT INTO tickets (
                ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                rules_version, patterns_version, inherited_from_parent
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'open', 'medium', ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at,
                     parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                     rules_version, patterns_version, inherited_from_parent
        "#,
        )
        .bind(&req.ticket_id)
        .bind(&req.project_id)
        .bind(&req.title)
        .bind(&execution_plan_json)
        .bind(&initial_stage)
        .bind(&req.parent_ticket_id)
        .bind(req.dependency_status.as_deref().unwrap_or("ready"))
        .bind(&req.created_by_worker_id)
        .bind(req.ticket_type.as_deref().unwrap_or("task"))
        .bind(project.rules_version.unwrap_or(1))
        .bind(project.patterns_version.unwrap_or(1))
        .bind(req.parent_ticket_id.is_some()) // inherited_from_parent
        .fetch_one(&mut *tx)
        .await?;

        // Add initial comment with description
        sqlx::query(
            r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, 'coordinator', 'coordinator', 0, ?2)
        "#,
        )
        .bind(&req.ticket_id)
        .bind(&req.description)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ticket)
    }

    pub async fn get_by_id(pool: &DbPool, ticket_id: &str) -> Result<Option<TicketWithComments>> {
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at
            FROM tickets
            WHERE ticket_id = ?1
        "#,
        )
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        if let Some(ticket) = ticket {
            let comments =
                crate::database::comments::Comment::get_by_ticket_id(pool, ticket_id).await?;
            Ok(Some(TicketWithComments { ticket, comments }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_by_project(
        pool: &DbPool,
        project_id: Option<&str>,
        status_filter: Option<&str>,
    ) -> Result<Vec<Ticket>> {
        let mut query = String::from(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at
            FROM tickets
        "#,
        );

        let mut conditions = Vec::new();

        if project_id.is_some() {
            conditions.push("project_id = ?1".to_string());
        }

        if status_filter.is_some() {
            let condition = if status_filter == Some("open") {
                "closed_at IS NULL".to_string()
            } else if status_filter == Some("closed") {
                "closed_at IS NOT NULL".to_string()
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

    pub async fn update_stage(
        pool: &DbPool,
        ticket_id: &str,
        new_stage: &str,
    ) -> Result<Option<Ticket>> {
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            UPDATE tickets 
            SET current_stage = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at
        "#,
        )
        .bind(new_stage)
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        Ok(ticket)
    }

    pub async fn close_ticket(
        pool: &DbPool,
        ticket_id: &str,
        status: &str,
    ) -> Result<Option<Ticket>> {
        let mut tx = pool.begin().await?;

        // Update ticket status
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            UPDATE tickets 
            SET current_stage = ?1, state = 'closed', updated_at = datetime('now'), closed_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at
        "#,
        )
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

            sqlx::query(
                r#"
                INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
                VALUES (?1, 'coordinator', 'coordinator', 999, ?2)
            "#,
            )
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

        if self.current_stage == "planning" {
            return Ok(plan.first().cloned());
        }

        // Find current stage index and return next
        for (i, stage) in plan.iter().enumerate() {
            if stage == &self.current_stage {
                return Ok(plan.get(i + 1).cloned());
            }
        }

        Ok(None)
    }

    pub fn is_completed(&self) -> bool {
        self.closed_at.is_some()
    }

    pub async fn update_state(
        pool: &DbPool,
        ticket_id: &str,
        state: &str,
    ) -> Result<Option<Ticket>> {
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            UPDATE tickets 
            SET state = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at
        "#,
        )
        .bind(state)
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        Ok(ticket)
    }

    pub async fn update_priority(
        pool: &DbPool,
        ticket_id: &str,
        priority: &str,
    ) -> Result<Option<Ticket>> {
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            UPDATE tickets 
            SET priority = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at
        "#,
        )
        .bind(priority)
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        Ok(ticket)
    }

    pub async fn get_by_stage_unclaimed(
        pool: &DbPool,
        project_id: &str,
        stage: &str,
    ) -> Result<Vec<Ticket>> {
        let tickets = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at
            FROM tickets
            WHERE project_id = ?1 
              AND current_stage = ?2 
              AND processing_worker_id IS NULL 
              AND state = 'open'
            ORDER BY priority DESC, created_at ASC
        "#,
        )
        .bind(project_id)
        .bind(stage)
        .fetch_all(pool)
        .await?;

        Ok(tickets)
    }

    pub async fn claim_for_processing(
        pool: &DbPool,
        ticket_id: &str,
        worker_id: &str,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2 
              AND processing_worker_id IS NULL 
              AND state = 'open'
        "#,
        )
        .bind(worker_id)
        .bind(ticket_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get a ticket with project rules and patterns included
    pub async fn get_with_project_info(
        pool: &DbPool,
        ticket_id: &str,
    ) -> Result<Option<TicketWithProjectInfo>> {
        let result = sqlx::query(
            r#"
            SELECT t.ticket_id, t.project_id, t.title, t.execution_plan, t.current_stage,
                   t.state, t.priority, t.processing_worker_id, t.created_at, t.updated_at, t.closed_at,
                   p.rules, p.patterns
            FROM tickets t
            LEFT JOIN projects p ON t.project_id = p.repository_name
            WHERE t.ticket_id = ?1
        "#,
        )
        .bind(ticket_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = result {
            let ticket = Ticket {
                ticket_id: row.get("ticket_id"),
                project_id: row.get("project_id"),
                title: row.get("title"),
                execution_plan: row.get("execution_plan"),
                current_stage: row.get("current_stage"),
                state: row.get("state"),
                priority: row.get("priority"),
                processing_worker_id: row.get("processing_worker_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                closed_at: row.get("closed_at"),
                parent_ticket_id: row.get("parent_ticket_id"),
                dependency_status: row.get("dependency_status"),
                created_by_worker_id: row.get("created_by_worker_id"),
                ticket_type: row.get("ticket_type"),
                rules_version: row.get("rules_version"),
                patterns_version: row.get("patterns_version"),
                inherited_from_parent: row.get("inherited_from_parent"),
            };

            let ticket_with_info = TicketWithProjectInfo {
                ticket,
                project_rules: row.get("rules"),
                project_patterns: row.get("patterns"),
            };

            Ok(Some(ticket_with_info))
        } else {
            Ok(None)
        }
    }

    /// Update dependency status of a ticket
    pub async fn update_dependency_status(
        pool: &DbPool,
        ticket_id: &str,
        new_status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tickets SET dependency_status = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
        )
        .bind(new_status)
        .bind(ticket_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get all child tickets of a parent ticket
    pub async fn get_children(pool: &DbPool, parent_ticket_id: &str) -> Result<Vec<Ticket>> {
        let tickets = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at,
                   parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                   rules_version, patterns_version, inherited_from_parent
            FROM tickets
            WHERE parent_ticket_id = ?1
            ORDER BY created_at ASC
        "#,
        )
        .bind(parent_ticket_id)
        .fetch_all(pool)
        .await?;

        Ok(tickets)
    }

    /// Get all tickets that are ready to process (dependency_status = 'ready' and state = 'open')
    pub async fn get_ready_tickets(pool: &DbPool, project_id: Option<&str>) -> Result<Vec<Ticket>> {
        let tickets = if let Some(project_id) = project_id {
            sqlx::query_as::<_, Ticket>(
                r#"
                SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                       processing_worker_id, created_at, updated_at, closed_at,
                       parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                       rules_version, patterns_version, inherited_from_parent
                FROM tickets
                WHERE project_id = ?1 AND dependency_status = 'ready' AND state = 'open'
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
            .bind(project_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, Ticket>(
                r#"
                SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                       processing_worker_id, created_at, updated_at, closed_at,
                       parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                       rules_version, patterns_version, inherited_from_parent
                FROM tickets
                WHERE dependency_status = 'ready' AND state = 'open'
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
            .fetch_all(pool)
            .await?
        };

        Ok(tickets)
    }

    /// Get all blocked tickets (dependency_status = 'blocked' and state = 'open')
    pub async fn get_blocked_tickets(
        pool: &DbPool,
        project_id: Option<&str>,
    ) -> Result<Vec<Ticket>> {
        let tickets = if let Some(project_id) = project_id {
            sqlx::query_as::<_, Ticket>(
                r#"
                SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                       processing_worker_id, created_at, updated_at, closed_at,
                       parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                       rules_version, patterns_version, inherited_from_parent
                FROM tickets
                WHERE project_id = ?1 AND dependency_status = 'blocked' AND state = 'open'
                ORDER BY created_at ASC
            "#,
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, Ticket>(
                r#"
                SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                       processing_worker_id, created_at, updated_at, closed_at,
                       parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                       rules_version, patterns_version, inherited_from_parent
                FROM tickets
                WHERE dependency_status = 'blocked' AND state = 'open'
                ORDER BY created_at ASC
            "#,
            )
            .fetch_all(pool)
            .await?
        };

        Ok(tickets)
    }
}
