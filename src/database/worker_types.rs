use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkerType {
    pub id: i64,
    pub project_id: String,
    pub worker_type: String,
    pub short_description: Option<String>,
    pub system_prompt: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkerTypeRequest {
    pub project_id: String,
    pub worker_type: String,
    pub short_description: Option<String>,
    pub system_prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkerTypeRequest {
    pub short_description: Option<String>,
    pub system_prompt: Option<String>,
}

impl WorkerType {
    pub async fn create(pool: &DbPool, req: CreateWorkerTypeRequest) -> Result<WorkerType> {
        let worker_type = sqlx::query_as::<_, WorkerType>(r#"
            INSERT INTO worker_types (project_id, worker_type, short_description, system_prompt)
            VALUES (?1, ?2, ?3, ?4)
            RETURNING id, project_id, worker_type, short_description, system_prompt, created_at, updated_at
        "#)
        .bind(&req.project_id)
        .bind(&req.worker_type)
        .bind(&req.short_description)
        .bind(&req.system_prompt)
        .fetch_one(pool)
        .await?;

        Ok(worker_type)
    }

    pub async fn get_by_type(
        pool: &DbPool,
        project_id: &str,
        worker_type: &str,
    ) -> Result<Option<WorkerType>> {
        let worker_type = sqlx::query_as::<_, WorkerType>(r#"
            SELECT id, project_id, worker_type, short_description, system_prompt, created_at, updated_at
            FROM worker_types
            WHERE project_id = ?1 AND worker_type = ?2
        "#)
        .bind(project_id)
        .bind(worker_type)
        .fetch_optional(pool)
        .await?;

        Ok(worker_type)
    }

    pub async fn list_by_project(
        pool: &DbPool,
        project_id: Option<&str>,
    ) -> Result<Vec<WorkerType>> {
        let worker_types = if let Some(project_id) = project_id {
            sqlx::query_as::<_, WorkerType>(r#"
                SELECT id, project_id, worker_type, short_description, system_prompt, created_at, updated_at
                FROM worker_types
                WHERE project_id = ?1
                ORDER BY created_at DESC
            "#)
            .bind(project_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, WorkerType>(r#"
                SELECT id, project_id, worker_type, short_description, system_prompt, created_at, updated_at
                FROM worker_types
                ORDER BY project_id ASC, created_at DESC
            "#)
            .fetch_all(pool)
            .await?
        };

        Ok(worker_types)
    }

    pub async fn update(
        pool: &DbPool,
        project_id: &str,
        worker_type: &str,
        req: UpdateWorkerTypeRequest,
    ) -> Result<Option<WorkerType>> {
        // Check if any updates are needed
        if req.short_description.is_none() && req.system_prompt.is_none() {
            return Self::get_by_type(pool, project_id, worker_type).await;
        }

        // Build update query dynamically
        let mut set_clauses = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(ref desc) = req.short_description {
            set_clauses.push("short_description = ?");
            bind_values.push(desc.clone());
        }
        if let Some(ref prompt) = req.system_prompt {
            set_clauses.push("system_prompt = ?");
            bind_values.push(prompt.clone());
        }

        set_clauses.push("updated_at = datetime('now')");

        let query = format!(
            "UPDATE worker_types SET {} WHERE project_id = ? AND worker_type = ? RETURNING id, project_id, worker_type, short_description, system_prompt, created_at, updated_at",
            set_clauses.join(", ")
        );

        let mut query_builder = sqlx::query_as::<_, WorkerType>(&query);

        // Bind values in order
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }
        query_builder = query_builder.bind(project_id);
        query_builder = query_builder.bind(worker_type);

        let worker_type = query_builder.fetch_optional(pool).await?;
        Ok(worker_type)
    }

    pub async fn delete(pool: &DbPool, project_id: &str, worker_type: &str) -> Result<bool> {
        let result =
            sqlx::query("DELETE FROM worker_types WHERE project_id = ?1 AND worker_type = ?2")
                .bind(project_id)
                .bind(worker_type)
                .execute(pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }
}
