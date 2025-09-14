// use chrono::{DateTime, Utc}; // For future datetime parsing if needed
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub repository_name: String,
    pub path: String,
    pub short_description: Option<String>,
    pub project_rules: Option<String>,
    pub project_patterns: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub repository_name: String,
    pub path: String,
    pub short_description: Option<String>,
    pub project_rules: Option<String>,
    pub project_patterns: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub path: Option<String>,
    pub short_description: Option<String>,
    pub project_rules: Option<String>,
    pub project_patterns: Option<String>,
}

impl Project {
    pub async fn create(pool: &DbPool, req: CreateProjectRequest) -> Result<Project> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (repository_name, path, short_description, project_rules, project_patterns)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING repository_name, path, short_description, project_rules, project_patterns, created_at, updated_at
        "#,
        )
        .bind(&req.repository_name)
        .bind(&req.path)
        .bind(&req.short_description)
        .bind(&req.project_rules)
        .bind(&req.project_patterns)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn get_by_name(pool: &DbPool, repository_name: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT repository_name, path, short_description, project_rules, project_patterns, created_at, updated_at
            FROM projects
            WHERE repository_name = ?1
        "#,
        )
        .bind(repository_name)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn list_all(pool: &DbPool) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT repository_name, path, short_description, project_rules, project_patterns, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC
        "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(projects)
    }

    pub async fn update(
        pool: &DbPool,
        repository_name: &str,
        req: UpdateProjectRequest,
    ) -> Result<Option<Project>> {
        // Check if any updates are needed
        if req.path.is_none() && req.short_description.is_none() && req.project_rules.is_none() && req.project_patterns.is_none() {
            return Self::get_by_name(pool, repository_name).await;
        }

        // Build update query dynamically
        let mut set_clauses = Vec::new();
        let mut bind_values: Vec<&str> = Vec::new();

        if let Some(ref path) = req.path {
            set_clauses.push("path = ?");
            bind_values.push(path);
        }
        if let Some(ref desc) = req.short_description {
            set_clauses.push("short_description = ?");
            bind_values.push(desc);
        }
        if let Some(ref rules) = req.project_rules {
            set_clauses.push("project_rules = ?");
            bind_values.push(rules);
        }
        if let Some(ref patterns) = req.project_patterns {
            set_clauses.push("project_patterns = ?");
            bind_values.push(patterns);
        }

        set_clauses.push("updated_at = datetime('now')");

        let query = format!(
            "UPDATE projects SET {} WHERE repository_name = ? RETURNING repository_name, path, short_description, project_rules, project_patterns, created_at, updated_at",
            set_clauses.join(", ")
        );

        let mut query_builder = sqlx::query_as::<_, Project>(&query);

        // Bind values in order
        for value in bind_values {
            query_builder = query_builder.bind(value);
        }
        query_builder = query_builder.bind(repository_name);

        let project = query_builder.fetch_optional(pool).await?;
        Ok(project)
    }

    pub async fn delete(pool: &DbPool, repository_name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM projects WHERE repository_name = ?1")
            .bind(repository_name)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
