//! Prompt repository implementation

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;
use vibe_ensemble_core::prompt::{PromptType, PromptVariable, SystemPrompt};

/// Repository for system prompt entities
pub struct PromptRepository {
    pool: Pool<Sqlite>,
}

impl PromptRepository {
    /// Create a new prompt repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new system prompt
    pub async fn create(&self, prompt: &SystemPrompt) -> Result<()> {
        let variables_json = serde_json::to_string(&prompt.variables)?;
        let prompt_type_str = self.prompt_type_to_string(&prompt.prompt_type);

        sqlx::query(
            r#"
            INSERT INTO system_prompts (
                id, name, description, template, prompt_type, variables,
                created_by, created_at, updated_at, version, is_active
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(prompt.id.to_string())
        .bind(&prompt.name)
        .bind(&prompt.description)
        .bind(&prompt.template)
        .bind(prompt_type_str)
        .bind(variables_json)
        .bind(prompt.created_by.to_string())
        .bind(prompt.created_at.to_rfc3339())
        .bind(prompt.updated_at.to_rfc3339())
        .bind(prompt.version as i64)
        .bind(prompt.is_active)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find a system prompt by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<SystemPrompt>> {
        let row = sqlx::query("SELECT * FROM system_prompts WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_prompt(row)?)),
            None => Ok(None),
        }
    }

    /// Update a system prompt
    pub async fn update(&self, prompt: &SystemPrompt) -> Result<()> {
        let variables_json = serde_json::to_string(&prompt.variables)?;
        let prompt_type_str = self.prompt_type_to_string(&prompt.prompt_type);

        sqlx::query(
            r#"
            UPDATE system_prompts SET
                name = ?, description = ?, template = ?, prompt_type = ?,
                variables = ?, updated_at = ?, version = ?, is_active = ?
            WHERE id = ?
            "#,
        )
        .bind(&prompt.name)
        .bind(&prompt.description)
        .bind(&prompt.template)
        .bind(prompt_type_str)
        .bind(variables_json)
        .bind(prompt.updated_at.to_rfc3339())
        .bind(prompt.version as i64)
        .bind(prompt.is_active)
        .bind(prompt.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a system prompt
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM system_prompts WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// List active system prompts
    pub async fn list_active(&self) -> Result<Vec<SystemPrompt>> {
        let rows = sqlx::query(
            "SELECT * FROM system_prompts WHERE is_active = 1 ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(self.row_to_prompt(row)?);
        }
        Ok(prompts)
    }

    /// Find prompts by type
    pub async fn find_by_type(&self, prompt_type: &PromptType) -> Result<Vec<SystemPrompt>> {
        let prompt_type_str = self.prompt_type_to_string(prompt_type);

        let rows = sqlx::query(
            "SELECT * FROM system_prompts WHERE prompt_type = ? AND is_active = 1 ORDER BY version DESC, created_at DESC"
        )
        .bind(prompt_type_str)
        .fetch_all(&self.pool)
        .await?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(self.row_to_prompt(row)?);
        }
        Ok(prompts)
    }

    /// Find prompts by name pattern
    pub async fn find_by_name_pattern(&self, pattern: &str) -> Result<Vec<SystemPrompt>> {
        let rows = sqlx::query(
            "SELECT * FROM system_prompts WHERE name LIKE ? AND is_active = 1 ORDER BY created_at DESC"
        )
        .bind(format!("%{}%", pattern))
        .fetch_all(&self.pool)
        .await?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(self.row_to_prompt(row)?);
        }
        Ok(prompts)
    }

    /// Get all versions of a prompt by name
    pub async fn find_versions_by_name(&self, name: &str) -> Result<Vec<SystemPrompt>> {
        let rows = sqlx::query("SELECT * FROM system_prompts WHERE name = ? ORDER BY version DESC")
            .bind(name)
            .fetch_all(&self.pool)
            .await?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(self.row_to_prompt(row)?);
        }
        Ok(prompts)
    }

    /// Get the latest version of a prompt by name
    pub async fn find_latest_by_name(&self, name: &str) -> Result<Option<SystemPrompt>> {
        let row = sqlx::query(
            "SELECT * FROM system_prompts WHERE name = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_prompt(row)?)),
            None => Ok(None),
        }
    }

    /// Count system prompts
    pub async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM system_prompts")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Count active system prompts
    pub async fn count_active(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM system_prompts WHERE is_active = 1")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Count prompts by type
    pub async fn count_by_type(&self, prompt_type: &PromptType) -> Result<i64> {
        let prompt_type_str = self.prompt_type_to_string(prompt_type);

        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM system_prompts WHERE prompt_type = ? AND is_active = 1",
        )
        .bind(prompt_type_str)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    // Helper methods

    fn prompt_type_to_string(&self, prompt_type: &PromptType) -> String {
        match prompt_type {
            PromptType::Coordinator => "coordinator".to_string(),
            PromptType::Worker => "worker".to_string(),
            PromptType::Specialist { domain } => format!("specialist:{}", domain),
            PromptType::Universal => "universal".to_string(),
        }
    }

    fn string_to_prompt_type(&self, s: &str) -> Result<PromptType> {
        if s == "coordinator" {
            Ok(PromptType::Coordinator)
        } else if s == "worker" {
            Ok(PromptType::Worker)
        } else if s == "universal" {
            Ok(PromptType::Universal)
        } else if s.starts_with("specialist:") {
            let domain = s.strip_prefix("specialist:").unwrap_or("").to_string();
            Ok(PromptType::Specialist { domain })
        } else {
            Err(crate::Error::Validation {
                message: format!("Invalid prompt type: {}", s),
            })
        }
    }

    fn row_to_prompt(&self, row: sqlx::sqlite::SqliteRow) -> Result<SystemPrompt> {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let description: String = row.get("description");
        let template: String = row.get("template");
        let prompt_type_str: String = row.get("prompt_type");
        let variables_json: String = row.get("variables");
        let created_by: String = row.get("created_by");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");
        let version: i64 = row.get("version");
        let is_active: bool = row.get("is_active");

        let id = Uuid::parse_str(&id)?;
        let created_by = Uuid::parse_str(&created_by)?;
        let created_at: DateTime<Utc> = created_at_str.parse()?;
        let updated_at: DateTime<Utc> = updated_at_str.parse()?;
        let prompt_type = self.string_to_prompt_type(&prompt_type_str)?;
        let variables: Vec<PromptVariable> = serde_json::from_str(&variables_json)?;

        Ok(SystemPrompt {
            id,
            name,
            description,
            template,
            prompt_type,
            variables,
            created_by,
            created_at,
            updated_at,
            version: version as u32,
            is_active,
        })
    }
}
