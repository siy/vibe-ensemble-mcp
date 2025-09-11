use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Worker {
    pub worker_id: String,
    pub project_id: String,
    pub worker_type: String,
    pub status: String,
    pub pid: Option<u32>,
    pub queue_name: String,
    pub started_at: String,
    pub last_activity: String,
}

impl Worker {
    pub async fn create(pool: &DbPool, worker: Worker) -> Result<Worker> {
        let worker = sqlx::query_as::<_, Worker>(r#"
            INSERT INTO workers (worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity
        "#)
        .bind(&worker.worker_id)
        .bind(&worker.project_id)
        .bind(&worker.worker_type)
        .bind(&worker.status)
        .bind(worker.pid.map(|p| p as i64))
        .bind(&worker.queue_name)
        .bind(&worker.started_at)
        .bind(&worker.last_activity)
        .fetch_one(pool)
        .await?;

        Ok(worker)
    }

    pub async fn get_by_id(pool: &DbPool, worker_id: &str) -> Result<Option<Worker>> {
        let worker = sqlx::query_as::<_, Worker>(
            r#"
            SELECT worker_id, project_id, worker_type, status, 
                   CAST(pid AS INTEGER) as pid, queue_name, started_at, last_activity
            FROM workers
            WHERE worker_id = ?1
        "#,
        )
        .bind(worker_id)
        .fetch_optional(pool)
        .await?;

        Ok(worker)
    }

    pub async fn list_by_project(pool: &DbPool, project_id: Option<&str>) -> Result<Vec<Worker>> {
        let workers = if let Some(project_id) = project_id {
            sqlx::query_as::<_, Worker>(
                r#"
                SELECT worker_id, project_id, worker_type, status, 
                       CAST(pid AS INTEGER) as pid, queue_name, started_at, last_activity
                FROM workers
                WHERE project_id = ?1
                ORDER BY started_at DESC
            "#,
            )
            .bind(project_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, Worker>(
                r#"
                SELECT worker_id, project_id, worker_type, status, 
                       CAST(pid AS INTEGER) as pid, queue_name, started_at, last_activity
                FROM workers
                ORDER BY project_id ASC, started_at DESC
            "#,
            )
            .fetch_all(pool)
            .await?
        };

        Ok(workers)
    }

    pub async fn list_by_type(pool: &DbPool, worker_type: &str) -> Result<Vec<Worker>> {
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT worker_id, project_id, worker_type, status, 
                   CAST(pid AS INTEGER) as pid, queue_name, started_at, last_activity
            FROM workers
            WHERE worker_type = ?1
            ORDER BY started_at DESC
        "#,
        )
        .bind(worker_type)
        .fetch_all(pool)
        .await?;

        Ok(workers)
    }

    pub async fn update_status(
        pool: &DbPool,
        worker_id: &str,
        status: &str,
        pid: Option<u32>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET status = ?1, pid = ?2, last_activity = datetime('now')
            WHERE worker_id = ?3
        "#,
        )
        .bind(status)
        .bind(pid.map(|p| p as i64))
        .bind(worker_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn update_last_activity(pool: &DbPool, worker_id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET last_activity = datetime('now')
            WHERE worker_id = ?1
        "#,
        )
        .bind(worker_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &DbPool, worker_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM workers WHERE worker_id = ?1")
            .bind(worker_id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
