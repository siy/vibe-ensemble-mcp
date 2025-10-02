use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, warn};

use super::DbPool;
use crate::events::EventType;

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
        event_type: EventType,
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
        .bind(event_type.to_string())
        .bind(ticket_id)
        .bind(worker_id)
        .bind(stage)
        .bind(reason)
        .fetch_one(pool)
        .await
        .inspect_err(|e| error!("Failed to create event of type '{}': {:?}", event_type, e))?;

        Ok(event)
    }

    pub async fn create_stage_completed(
        pool: &DbPool,
        ticket_id: &str,
        stage: &str,
        worker_id: &str,
    ) -> Result<Event> {
        Self::create(
            pool,
            EventType::StageCompleted,
            Some(ticket_id),
            Some(worker_id),
            Some(stage),
            None,
        )
        .await
    }

    pub async fn create_worker_stopped(
        pool: &DbPool,
        worker_id: &str,
        reason: &str,
    ) -> Result<Event> {
        Self::create(
            pool,
            EventType::WorkerStopped,
            None,
            Some(worker_id),
            None,
            Some(reason),
        )
        .await
    }

    pub async fn create_task_assigned(
        pool: &DbPool,
        ticket_id: &str,
        queue_name: &str,
    ) -> Result<Event> {
        Self::create(
            pool,
            EventType::TaskAssigned,
            Some(ticket_id),
            None,
            None,
            Some(queue_name),
        )
        .await
    }

    pub async fn get_recent(pool: &DbPool, limit: i32) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
            FROM events
            ORDER BY id DESC
            LIMIT ?1
        "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .inspect_err(|e| warn!("Failed to fetch recent events: {:?}", e))?;

        Ok(events)
    }

    pub async fn get_unprocessed(pool: &DbPool) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
            FROM events
            WHERE processed = 0
            ORDER BY id ASC
        "#,
        )
        .fetch_all(pool)
        .await
        .inspect_err(|e| warn!("Failed to fetch unprocessed events: {:?}", e))?;

        Ok(events)
    }

    pub async fn get_all(pool: &DbPool, processed_filter: Option<bool>) -> Result<Vec<Event>> {
        let events = match processed_filter {
            Some(processed) => {
                sqlx::query_as::<_, Event>(r#"
                    SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
                    FROM events
                    WHERE processed = ?1
                    ORDER BY id ASC
                "#)
                .bind(processed)
                .fetch_all(pool)
                .await
                .inspect_err(|e| warn!("Failed to fetch all events with filter: {:?}", e))?
            }
            None => {
                sqlx::query_as::<_, Event>(r#"
                    SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
                    FROM events
                    ORDER BY id ASC
                "#)
                .fetch_all(pool)
                .await
                .inspect_err(|e| warn!("Failed to fetch all events: {:?}", e))?
            }
        };

        Ok(events)
    }

    pub async fn get_by_ids(pool: &DbPool, event_ids: &[i64]) -> Result<Vec<Event>> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = event_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        let query = format!(
            r#"
            SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed, resolution_summary
            FROM events
            WHERE id IN ({})
            ORDER BY id ASC
        "#,
            placeholders
        );

        let mut query_builder = sqlx::query_as::<_, Event>(&query);
        for id in event_ids {
            query_builder = query_builder.bind(id);
        }

        let events = query_builder
            .fetch_all(pool)
            .await
            .inspect_err(|e| warn!("Failed to fetch events by IDs: {:?}", e))?;
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

        let result = query_builder
            .execute(pool)
            .await
            .inspect_err(|e| error!("Failed to mark events as processed: {:?}", e))?;
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
        .await
        .inspect_err(|e| error!("Failed to resolve event {}: {:?}", event_id, e))?;

        Ok(())
    }
}
