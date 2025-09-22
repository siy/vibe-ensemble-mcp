use anyhow::Result;
use tracing::{debug, info};

use crate::database::DbPool;

/// Manages ticket state transitions and stage progressions
pub struct TicketTransitionManager {
    db: DbPool,
}

impl TicketTransitionManager {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// Transition a ticket to a specific stage
    pub async fn transition_to_stage(
        &self,
        ticket_id: &str,
        target_stage: &str,
        comment: &str,
    ) -> Result<()> {
        debug!(
            ticket_id = %ticket_id,
            target_stage = %target_stage,
            "Transitioning ticket to stage"
        );

        // Update ticket stage and add comment
        sqlx::query!(
            r#"
            UPDATE tickets
            SET current_stage = ?1,
                state = 'open',
                processing_worker_id = NULL,
                updated_at = datetime('now')
            WHERE ticket_id = ?2
            "#,
            target_stage,
            ticket_id
        )
        .execute(&self.db)
        .await?;

        // Add comment about the transition
        let comment_text = format!("Stage transition: {}", comment);
        sqlx::query!(
            r#"
            INSERT INTO comments (comment_id, ticket_id, content, created_at)
            VALUES (?1, ?2, ?3, datetime('now'))
            "#,
            uuid::Uuid::new_v4().to_string(),
            ticket_id,
            comment_text
        )
        .execute(&self.db)
        .await?;

        info!(
            ticket_id = %ticket_id,
            target_stage = %target_stage,
            "Successfully transitioned ticket to stage"
        );

        Ok(())
    }

    /// Place a ticket on hold for coordinator attention
    pub async fn place_on_hold(&self, ticket_id: &str, reason: &str) -> Result<()> {
        debug!(
            ticket_id = %ticket_id,
            reason = %reason,
            "Placing ticket on hold"
        );

        // Update ticket state to on_hold
        sqlx::query!(
            r#"
            UPDATE tickets
            SET state = 'on_hold',
                processing_worker_id = NULL,
                updated_at = datetime('now')
            WHERE ticket_id = ?1
            "#,
            ticket_id
        )
        .execute(&self.db)
        .await?;

        // Add comment about being placed on hold
        let comment_text = format!("Placed on hold: {}", reason);
        sqlx::query!(
            r#"
            INSERT INTO comments (comment_id, ticket_id, content, created_at)
            VALUES (?1, ?2, ?3, datetime('now'))
            "#,
            uuid::Uuid::new_v4().to_string(),
            ticket_id,
            comment_text
        )
        .execute(&self.db)
        .await?;

        info!(
            ticket_id = %ticket_id,
            "Successfully placed ticket on hold"
        );

        Ok(())
    }

    /// Get the next stage for a ticket based on its project pipeline
    pub async fn get_next_stage(&self, ticket_id: &str) -> Result<Option<String>> {
        let result = sqlx::query!(
            r#"
            SELECT p.pipeline, t.current_stage
            FROM tickets t
            JOIN projects p ON t.project_id = p.project_id
            WHERE t.ticket_id = ?1
            "#,
            ticket_id
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = result {
            let pipeline: Vec<String> = serde_json::from_str(&row.pipeline)?;
            let current_stage = row.current_stage;

            // Find current stage index and return next stage
            if let Some(current_index) = pipeline.iter().position(|s| s == &current_stage) {
                if current_index + 1 < pipeline.len() {
                    return Ok(Some(pipeline[current_index + 1].clone()));
                }
            }
        }

        Ok(None) // No next stage or ticket not found
    }

    /// Get the previous stage for a ticket based on its project pipeline
    pub async fn get_previous_stage(&self, ticket_id: &str) -> Result<Option<String>> {
        let result = sqlx::query!(
            r#"
            SELECT p.pipeline, t.current_stage
            FROM tickets t
            JOIN projects p ON t.project_id = p.project_id
            WHERE t.ticket_id = ?1
            "#,
            ticket_id
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = result {
            let pipeline: Vec<String> = serde_json::from_str(&row.pipeline)?;
            let current_stage = row.current_stage;

            // Find current stage index and return previous stage
            if let Some(current_index) = pipeline.iter().position(|s| s == &current_stage) {
                if current_index > 0 {
                    return Ok(Some(pipeline[current_index - 1].clone()));
                }
            }
        }

        Ok(None) // No previous stage or ticket not found
    }

    /// Get basic ticket information (project_id, current_stage)
    pub async fn get_ticket_info(&self, ticket_id: &str) -> Result<(String, String)> {
        let result = sqlx::query!(
            r#"
            SELECT project_id, current_stage
            FROM tickets
            WHERE ticket_id = ?1
            "#,
            ticket_id
        )
        .fetch_one(&self.db)
        .await?;

        Ok((result.project_id, result.current_stage))
    }

    /// Validate that a stage transition is allowed (pipeline immutability)
    pub async fn validate_stage_transition(
        &self,
        ticket_id: &str,
        target_stage: &str,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            SELECT p.pipeline, t.current_stage
            FROM tickets t
            JOIN projects p ON t.project_id = p.project_id
            WHERE t.ticket_id = ?1
            "#,
            ticket_id
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = result {
            let pipeline: Vec<String> = serde_json::from_str(&row.pipeline)?;
            let current_stage = row.current_stage;

            let current_index = pipeline.iter().position(|s| s == &current_stage);
            let target_index = pipeline.iter().position(|s| s == target_stage);

            if let (Some(current_idx), Some(target_idx)) = (current_index, target_index) {
                // Allow forward movement, backward movement, or staying in same stage
                // Pipeline immutability: can't skip stages, but can move backwards for rework
                return Ok(target_idx <= current_idx + 1);
            }
        }

        // If we can't find the stages in the pipeline, allow the transition
        // This handles cases where stages might be added dynamically
        Ok(true)
    }

    /// Mark a ticket as completed
    pub async fn mark_completed(&self, ticket_id: &str, final_comment: &str) -> Result<()> {
        debug!(
            ticket_id = %ticket_id,
            "Marking ticket as completed"
        );

        // Update ticket to completed state
        sqlx::query!(
            r#"
            UPDATE tickets
            SET state = 'closed',
                current_stage = 'completed',
                processing_worker_id = NULL,
                updated_at = datetime('now')
            WHERE ticket_id = ?1
            "#,
            ticket_id
        )
        .execute(&self.db)
        .await?;

        // Add final comment
        let comment_text = format!("Completed: {}", final_comment);
        sqlx::query!(
            r#"
            INSERT INTO comments (comment_id, ticket_id, content, created_at)
            VALUES (?1, ?2, ?3, datetime('now'))
            "#,
            uuid::Uuid::new_v4().to_string(),
            ticket_id,
            comment_text
        )
        .execute(&self.db)
        .await?;

        info!(
            ticket_id = %ticket_id,
            "Successfully marked ticket as completed"
        );

        Ok(())
    }
}