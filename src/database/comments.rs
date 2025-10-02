use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, warn};

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
    pub async fn create(
        pool: &DbPool,
        ticket_id: &str,
        worker_type: Option<&str>,
        worker_id: Option<&str>,
        stage_number: Option<i32>,
        content: &str,
    ) -> Result<Comment> {
        let comment = sqlx::query_as::<_, Comment>(
            r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, ticket_id, worker_type, worker_id, stage_number, content, created_at
        "#,
        )
        .bind(ticket_id)
        .bind(worker_type)
        .bind(worker_id)
        .bind(stage_number)
        .bind(content)
        .fetch_one(pool)
        .await
        .inspect_err(|e| {
            error!(
                "Failed to create comment for ticket '{}': {:?}",
                ticket_id, e
            )
        })?;

        Ok(comment)
    }

    pub async fn create_from_request(pool: &DbPool, req: CreateCommentRequest) -> Result<Comment> {
        let comment = sqlx::query_as::<_, Comment>(
            r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, ticket_id, worker_type, worker_id, stage_number, content, created_at
        "#,
        )
        .bind(&req.ticket_id)
        .bind(&req.worker_type)
        .bind(&req.worker_id)
        .bind(req.stage_number)
        .bind(&req.content)
        .fetch_one(pool)
        .await
        .inspect_err(|e| {
            error!(
                "Failed to create comment from request for ticket '{}': {:?}",
                req.ticket_id, e
            )
        })?;

        Ok(comment)
    }

    pub async fn get_by_ticket_id(pool: &DbPool, ticket_id: &str) -> Result<Vec<Comment>> {
        let comments = sqlx::query_as::<_, Comment>(
            r#"
            SELECT id, ticket_id, worker_type, worker_id, stage_number, content, created_at
            FROM comments
            WHERE ticket_id = ?1
            ORDER BY created_at ASC
        "#,
        )
        .bind(ticket_id)
        .fetch_all(pool)
        .await
        .inspect_err(|e| {
            warn!(
                "Failed to fetch comments for ticket '{}': {:?}",
                ticket_id, e
            )
        })?;

        Ok(comments)
    }

    pub async fn add_with_stage_update(
        pool: &DbPool,
        req: CreateCommentRequest,
        new_stage: &str,
    ) -> Result<(Comment, bool)> {
        let mut tx = pool.begin().await.inspect_err(|e| {
            error!(
                "Failed to begin transaction for comment with stage update for ticket '{}': {:?}",
                req.ticket_id, e
            )
        })?;

        // Add comment
        let comment = sqlx::query_as::<_, Comment>(
            r#"
            INSERT INTO comments (ticket_id, worker_type, worker_id, stage_number, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, ticket_id, worker_type, worker_id, stage_number, content, created_at
        "#,
        )
        .bind(&req.ticket_id)
        .bind(&req.worker_type)
        .bind(&req.worker_id)
        .bind(req.stage_number)
        .bind(&req.content)
        .fetch_one(&mut *tx)
        .await
        .inspect_err(|e| {
            error!(
                "Failed to insert comment for ticket '{}' in stage update: {:?}",
                req.ticket_id, e
            )
        })?;

        // Update ticket stage
        let updated_rows = sqlx::query(
            r#"
            UPDATE tickets
            SET current_stage = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
        "#,
        )
        .bind(new_stage)
        .bind(&req.ticket_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| {
            error!(
                "Failed to update stage to '{}' for ticket '{}': {:?}",
                new_stage, req.ticket_id, e
            )
        })?;

        tx.commit().await.inspect_err(|e| {
            error!(
                "Failed to commit transaction for comment with stage update for ticket '{}': {:?}",
                req.ticket_id, e
            )
        })?;

        Ok((comment, updated_rows.rows_affected() > 0))
    }
}
