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
    pub created_at: String,
    pub updated_at: String,
    // Renamed from project_rules/project_patterns (removing redundant prefix)
    pub rules: Option<String>,
    pub patterns: Option<String>,
    // New versioning fields for DAG support
    pub rules_version: Option<i32>,
    pub patterns_version: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub repository_name: String,
    pub path: String,
    pub short_description: Option<String>,
    pub rules: Option<String>,
    pub patterns: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub path: Option<String>,
    pub short_description: Option<String>,
    pub rules: Option<String>,
    pub patterns: Option<String>,
}

impl Project {
    pub async fn create(pool: &DbPool, req: CreateProjectRequest) -> Result<Project> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (repository_name, path, short_description, rules, patterns, rules_version, patterns_version)
            VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)
            RETURNING repository_name, path, short_description, created_at, updated_at, rules, patterns, rules_version, patterns_version
        "#,
        )
        .bind(&req.repository_name)
        .bind(&req.path)
        .bind(&req.short_description)
        .bind(&req.rules)
        .bind(&req.patterns)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn get_by_name(pool: &DbPool, repository_name: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            SELECT repository_name, path, short_description, rules, patterns, created_at, updated_at, rules_version, patterns_version
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
            SELECT repository_name, path, short_description, rules, patterns, created_at, updated_at, rules_version, patterns_version
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
        if req.path.is_none()
            && req.short_description.is_none()
            && req.rules.is_none()
            && req.patterns.is_none()
        {
            return Self::get_by_name(pool, repository_name).await;
        }

        // Build update query using QueryBuilder for safer parameter binding
        let mut query_builder = sqlx::QueryBuilder::new("UPDATE projects SET ");
        let mut has_field = false;

        if let Some(ref path) = req.path {
            if has_field {
                query_builder.push(", ");
            }
            query_builder.push("path = ");
            query_builder.push_bind(path);
            has_field = true;
        }
        if let Some(ref desc) = req.short_description {
            if has_field {
                query_builder.push(", ");
            }
            query_builder.push("short_description = ");
            query_builder.push_bind(desc);
            has_field = true;
        }
        if let Some(ref rules) = req.rules {
            if has_field {
                query_builder.push(", ");
            }
            query_builder.push("rules = ");
            query_builder.push_bind(rules);
            has_field = true;
        }
        if let Some(ref patterns) = req.patterns {
            if has_field {
                query_builder.push(", ");
            }
            query_builder.push("patterns = ");
            query_builder.push_bind(patterns);
            has_field = true;
        }

        if has_field {
            query_builder.push(", ");
        }
        query_builder.push("updated_at = datetime('now')");

        query_builder.push(" WHERE repository_name = ");
        query_builder.push_bind(repository_name);
        query_builder.push(" RETURNING repository_name, path, short_description, rules, patterns, created_at, updated_at, rules_version, patterns_version");

        let project = query_builder
            .build_query_as::<Project>()
            .fetch_optional(pool)
            .await?;
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
