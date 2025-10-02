use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, warn};

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
            INSERT OR REPLACE INTO workers (worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity)
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
        .await
        .inspect_err(|e| error!("Failed to create worker '{}': {:?}", worker.worker_id, e))?;

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
        .await
        .inspect_err(|e| warn!("Failed to fetch worker '{}': {:?}", worker_id, e))?;

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
            .await
            .inspect_err(|e| {
                warn!(
                    "Failed to list workers for project '{}': {:?}",
                    project_id, e
                )
            })?
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
            .await
            .inspect_err(|e| warn!("Failed to list all workers: {:?}", e))?
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
        .await
        .inspect_err(|e| warn!("Failed to list workers of type '{}': {:?}", worker_type, e))?;

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
        .await
        .inspect_err(|e| {
            error!(
                "Failed to update status for worker '{}' to '{}': {:?}",
                worker_id, status, e
            )
        })?;

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
        .await
        .inspect_err(|e| {
            warn!(
                "Failed to update last activity for worker '{}': {:?}",
                worker_id, e
            )
        })?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &DbPool, worker_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM workers WHERE worker_id = ?1")
            .bind(worker_id)
            .execute(pool)
            .await
            .inspect_err(|e| error!("Failed to delete worker '{}': {:?}", worker_id, e))?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn has_active_worker_for_queue(pool: &DbPool, queue_name: &str) -> Result<bool> {
        // Get workers that appear active in database
        let workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity
            FROM workers 
            WHERE queue_name = ?1 AND status IN ('spawning', 'active', 'idle')
        "#,
        )
        .bind(queue_name)
        .fetch_all(pool)
        .await
        .inspect_err(|e| warn!("Failed to fetch workers for queue '{}': {:?}", queue_name, e))?;

        // Check if any of the workers are actually running
        for worker in workers {
            if let Some(pid) = worker.pid {
                // Check if process is still running using kill -0
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);

                if is_running {
                    return Ok(true);
                } else {
                    // Process died, update its status to failed
                    tracing::warn!(
                        "Worker {} (PID {}) marked as {} but process is dead, updating status",
                        worker.worker_id,
                        pid,
                        worker.status
                    );
                    Self::update_status(pool, &worker.worker_id, "failed", None).await?;

                    // Create event for process death
                    crate::database::events::Event::create_worker_stopped(
                        pool,
                        &worker.worker_id,
                        "process died unexpectedly",
                    )
                    .await?;
                }
            } else if worker.status == "spawning" {
                // Workers in spawning state without PID might still be valid for a short time
                return Ok(true);
            }
        }

        Ok(false)
    }
}
