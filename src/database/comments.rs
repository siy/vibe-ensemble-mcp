use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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
        .await?;

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
        .await?;

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
        .await?;

        tx.commit().await?;

        Ok((comment, updated_rows.rows_affected() > 0))
    }
}
