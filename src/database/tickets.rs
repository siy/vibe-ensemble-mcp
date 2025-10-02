use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::fmt;

use super::DbPool;

/// Ticket state enum for type safety
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketState {
    Open,
    Closed,
    OnHold,
}

/// Dependency status enum for type safety
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Ready,
    Blocked,
}

/// Priority enum for type safety
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl fmt::Display for TicketState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TicketState::Open => write!(f, "open"),
            TicketState::Closed => write!(f, "closed"),
            TicketState::OnHold => write!(f, "on_hold"),
        }
    }
}

impl std::str::FromStr for TicketState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(TicketState::Open),
            "closed" => Ok(TicketState::Closed),
            "on_hold" => Ok(TicketState::OnHold),
            _ => Err(anyhow::anyhow!("Invalid ticket state: {}", s)),
        }
    }
}

impl fmt::Display for DependencyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyStatus::Ready => write!(f, "ready"),
            DependencyStatus::Blocked => write!(f, "blocked"),
        }
    }
}

impl std::str::FromStr for DependencyStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ready" => Ok(DependencyStatus::Ready),
            "blocked" => Ok(DependencyStatus::Blocked),
            _ => Err(anyhow::anyhow!("Invalid dependency status: {}", s)),
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Urgent => write!(f, "urgent"),
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            "urgent" => Ok(Priority::Urgent),
            _ => Err(anyhow::anyhow!("Invalid priority: {}", s)),
        }
    }
}

impl TicketState {
    /// Get all valid ticket states
    pub fn all() -> Vec<TicketState> {
        vec![TicketState::Open, TicketState::Closed, TicketState::OnHold]
    }

    /// Get all valid ticket state strings
    pub fn all_strings() -> Vec<&'static str> {
        vec!["open", "closed", "on_hold"]
    }

    /// Get the string representation for SQL queries (same as Display but explicit)
    pub fn as_sql_value(&self) -> &'static str {
        match self {
            TicketState::Open => "open",
            TicketState::Closed => "closed",
            TicketState::OnHold => "on_hold",
        }
    }
}

impl DependencyStatus {
    pub fn as_sql_value(&self) -> &'static str {
        match self {
            DependencyStatus::Ready => "ready",
            DependencyStatus::Blocked => "blocked",
        }
    }
}

impl Priority {
    pub fn as_sql_value(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Urgent => "urgent",
        }
    }
}

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
    pub priority: Option<String>,
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
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
        .bind(TicketState::Open.as_sql_value())
        .bind(req.priority.as_deref().unwrap_or("medium"))
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
                   processing_worker_id, created_at, updated_at, closed_at,
                   parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                   rules_version, patterns_version, inherited_from_parent
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
        use sqlx::QueryBuilder;

        // Validate status_filter with explicit check
        if let Some(status) = status_filter {
            if status != "open" && status != "closed" {
                return Err(anyhow::anyhow!("Invalid status filter: {}", status));
            }
        }

        // Use QueryBuilder for safe parameterized queries
        let mut query_builder = QueryBuilder::new(
            "SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                    processing_worker_id, created_at, updated_at, closed_at,
                    parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                    rules_version, patterns_version, inherited_from_parent
             FROM tickets WHERE 1=1",
        );

        if let Some(pid) = project_id {
            query_builder.push(" AND project_id = ");
            query_builder.push_bind(pid);
        }

        if let Some(status) = status_filter {
            match status {
                "open" => {
                    query_builder.push(" AND closed_at IS NULL");
                }
                "closed" => {
                    query_builder.push(" AND closed_at IS NOT NULL");
                }
                _ => unreachable!("status already validated"),
            }
        }

        query_builder.push(" ORDER BY created_at DESC");

        let tickets = query_builder
            .build_query_as::<Ticket>()
            .fetch_all(pool)
            .await?;
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
                     processing_worker_id, created_at, updated_at, closed_at,
                     parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                     rules_version, patterns_version, inherited_from_parent
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

        // Update ticket status and set dependency_status to 'ready' since closed tickets
        // have implicitly satisfied their dependencies
        let ticket = sqlx::query_as::<_, Ticket>(
            r#"
            UPDATE tickets
            SET current_stage = ?1, state = ?2, dependency_status = 'ready',
                updated_at = datetime('now'), closed_at = datetime('now')
            WHERE ticket_id = ?3
            RETURNING ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                     processing_worker_id, created_at, updated_at, closed_at,
                     parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                     rules_version, patterns_version, inherited_from_parent
        "#,
        )
        .bind(status)
        .bind(TicketState::Closed.as_sql_value())
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

    pub async fn place_on_hold(pool: &DbPool, ticket_id: &str, reason: &str) -> Result<()> {
        let mut tx = pool.begin().await?;

        // Update ticket state to on_hold and release processing worker
        sqlx::query(
            r#"
            UPDATE tickets
            SET state = ?1, processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?2
            "#,
        )
        .bind(TicketState::OnHold.as_sql_value())
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;

        // Add comment explaining why ticket is on hold
        sqlx::query(
            r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, 'system', 'system', 999, ?2)
            "#,
        )
        .bind(ticket_id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
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
                     processing_worker_id, created_at, updated_at, closed_at,
                     parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                     rules_version, patterns_version, inherited_from_parent
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
                     processing_worker_id, created_at, updated_at, closed_at,
                     parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                     rules_version, patterns_version, inherited_from_parent
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
                   processing_worker_id, created_at, updated_at, closed_at,
                   parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                   rules_version, patterns_version, inherited_from_parent
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
              AND dependency_status = 'ready'
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
                   t.parent_ticket_id, t.dependency_status, t.created_by_worker_id, t.ticket_type,
                   t.rules_version, t.patterns_version, t.inherited_from_parent,
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

    /// List open tickets by current stage with priority ordering
    pub async fn list_open_by_stage(pool: &DbPool, stage: &str) -> Result<Vec<Ticket>> {
        let tickets = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT ticket_id, project_id, title, execution_plan, current_stage, state, priority,
                   processing_worker_id, created_at, updated_at, closed_at,
                   parent_ticket_id, dependency_status, created_by_worker_id, ticket_type,
                   rules_version, patterns_version, inherited_from_parent
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
        .bind(stage)
        .fetch_all(pool)
        .await?;

        Ok(tickets)
    }

    /// Get the ticket state as an enum
    pub fn get_state(&self) -> Result<TicketState> {
        self.state.parse()
    }

    /// Get dependency status as typed enum
    pub fn get_dependency_status(&self) -> Result<DependencyStatus> {
        self.dependency_status.parse()
    }

    /// Get priority as typed enum
    pub fn get_priority(&self) -> Result<Priority> {
        self.priority.parse()
    }

    /// Check if ticket is open
    pub fn is_open(&self) -> bool {
        matches!(self.get_state().ok(), Some(TicketState::Open))
    }

    /// Check if ticket is closed
    pub fn is_closed(&self) -> bool {
        matches!(self.get_state().ok(), Some(TicketState::Closed))
    }

    /// Check if ticket is on hold
    pub fn is_on_hold(&self) -> bool {
        matches!(self.get_state().ok(), Some(TicketState::OnHold))
    }

    /// Check if ticket dependency status is ready
    pub fn is_dependency_ready(&self) -> bool {
        matches!(
            self.get_dependency_status().ok(),
            Some(DependencyStatus::Ready)
        )
    }

    /// Check if ticket dependency status is blocked
    pub fn is_dependency_blocked(&self) -> bool {
        matches!(
            self.get_dependency_status().ok(),
            Some(DependencyStatus::Blocked)
        )
    }
}
