use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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
    pub resolution_summary: Option<String>,
}

impl Event {
    pub async fn create(
        pool: &DbPool,
        event_type: &str,
        ticket_id: Option<&str>,
        worker_id: Option<&str>,
        stage: Option<&str>,
        reason: Option<&str>,
    ) -> Result<Event> {
        let event = sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO events (event_type, ticket_id, worker_id, stage, reason)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
        "#,
        )
        .bind(event_type)
        .bind(ticket_id)
        .bind(worker_id)
        .bind(stage)
        .bind(reason)
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn create_stage_completed(
        pool: &DbPool,
        ticket_id: &str,
        stage: &str,
        worker_id: &str,
    ) -> Result<Event> {
        let event = sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO events (event_type, ticket_id, worker_id, stage)
            VALUES ('ticket_stage_completed', ?1, ?2, ?3)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
        "#,
        )
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
        let event = sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO events (event_type, worker_id, reason)
            VALUES ('worker_stopped', ?1, ?2)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
        "#,
        )
        .bind(worker_id)
        .bind(reason)
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn create_task_assigned(
        pool: &DbPool,
        ticket_id: &str,
        queue_name: &str,
    ) -> Result<Event> {
        let event = sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO events (event_type, ticket_id, reason)
            VALUES ('task_assigned', ?1, ?2)
            RETURNING id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
        "#,
        )
        .bind(ticket_id)
        .bind(queue_name)
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn get_recent(pool: &DbPool, limit: i32) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
            FROM events
            ORDER BY created_at DESC
            LIMIT ?1
        "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(events)
    }

    pub async fn get_unprocessed(pool: &DbPool) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
            FROM events
            WHERE processed = 0
            ORDER BY created_at ASC
        "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(events)
    }

    pub async fn get_all(pool: &DbPool, processed_filter: Option<bool>) -> Result<Vec<Event>> {
        let events = match processed_filter {
            Some(processed) => {
                sqlx::query_as::<_, Event>(r#"
                    SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
                    FROM events
                    WHERE processed = ?1
                    ORDER BY created_at DESC
                "#)
                .bind(processed)
                .fetch_all(pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Event>(r#"
                    SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
                    FROM events
                    ORDER BY created_at DESC
                "#)
                .fetch_all(pool)
                .await?
            }
        };

        Ok(events)
    }

    pub async fn mark_processed(pool: &DbPool, event_ids: &[i64]) -> Result<u64> {
        if event_ids.is_empty() {
            return Ok(0);
        }

        let placeholders = event_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        let query = format!(
            r#"
            UPDATE events 
            SET processed = 1 
            WHERE id IN ({})
        "#,
            placeholders
        );

        let mut query_builder = sqlx::query(&query);
        for id in event_ids {
            query_builder = query_builder.bind(id);
        }

        let result = query_builder.execute(pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn resolve_event(
        pool: &DbPool,
        event_id: i64,
        resolution_summary: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE events 
            SET processed = 1, resolution_summary = ?1
            WHERE id = ?2
        "#,
        )
        .bind(resolution_summary)
        .bind(event_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}
