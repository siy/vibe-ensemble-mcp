//! Issue repository implementation

use crate::{Error, Result};
use anyhow;
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::{Pool, Sqlite};
use tracing::{debug, info};
use uuid::Uuid;
use vibe_ensemble_core::issue::{Issue, IssuePriority, IssueStatus};

/// Repository for issue entities
pub struct IssueRepository {
    pool: Pool<Sqlite>,
}

impl IssueRepository {
    /// Create a new issue repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new issue
    pub async fn create(&self, issue: &Issue) -> Result<()> {
        debug!("Creating issue: {} ({})", issue.title, issue.id);

        let tags_json = serde_json::to_string(&issue.tags)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize tags: {}", e)))?;

        let status_str = self.serialize_status(&issue.status);
        let priority_str = self.serialize_priority(&issue.priority);

        let assigned_agent_id_str = issue.assigned_agent_id.map(|id| id.to_string());
        let resolved_at_str = issue.resolved_at.map(|dt| dt.to_rfc3339());
        let issue_id_str = issue.id.to_string();
        let created_at_str = issue.created_at.to_rfc3339();
        let updated_at_str = issue.updated_at.to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO issues (id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            issue_id_str,
            issue.title,
            issue.description,
            status_str,
            priority_str,
            assigned_agent_id_str,
            created_at_str,
            updated_at_str,
            resolved_at_str,
            tags_json
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        info!("Successfully created issue: {} ({})", issue.title, issue.id);
        Ok(())
    }

    /// Find an issue by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Issue>> {
        debug!("Finding issue by ID: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues WHERE id = ?1",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let issue = self.parse_issue_from_row(
                    row.id.as_ref().unwrap(),
                    &row.title,
                    &row.description,
                    &row.status,
                    &row.priority,
                    row.assigned_agent_id.as_deref(),
                    &row.created_at,
                    &row.updated_at,
                    row.resolved_at.as_deref(),
                    &row.tags,
                )?;
                Ok(Some(issue))
            }
            None => Ok(None),
        }
    }

    /// Update an issue
    pub async fn update(&self, issue: &Issue) -> Result<()> {
        debug!("Updating issue: {} ({})", issue.title, issue.id);

        let tags_json = serde_json::to_string(&issue.tags)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize tags: {}", e)))?;

        let status_str = self.serialize_status(&issue.status);
        let priority_str = self.serialize_priority(&issue.priority);
        let assigned_agent_id_str = issue.assigned_agent_id.map(|id| id.to_string());
        let resolved_at_str = issue.resolved_at.map(|dt| dt.to_rfc3339());
        let issue_id_str = issue.id.to_string();
        let updated_at_str = issue.updated_at.to_rfc3339();

        let rows_affected = sqlx::query!(
            r#"
            UPDATE issues 
            SET title = ?2, description = ?3, status = ?4, priority = ?5, assigned_agent_id = ?6, updated_at = ?7, resolved_at = ?8, tags = ?9
            WHERE id = ?1
            "#,
            issue_id_str,
            issue.title,
            issue.description,
            status_str,
            priority_str,
            assigned_agent_id_str,
            updated_at_str,
            resolved_at_str,
            tags_json
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(Error::NotFound {
                entity: "Issue".to_string(),
                id: issue.id.to_string(),
            });
        }

        info!("Successfully updated issue: {} ({})", issue.title, issue.id);
        Ok(())
    }

    /// Delete an issue
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        debug!("Deleting issue with ID: {}", id);

        let id_str = id.to_string();
        let rows_affected = sqlx::query!("DELETE FROM issues WHERE id = ?1", id_str)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?
            .rows_affected();

        if rows_affected == 0 {
            return Err(Error::NotFound {
                entity: "Issue".to_string(),
                id: id.to_string(),
            });
        }

        info!("Successfully deleted issue with ID: {}", id);
        Ok(())
    }

    /// List all issues
    pub async fn list(&self) -> Result<Vec<Issue>> {
        debug!("Listing all issues");

        let rows = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut issues = Vec::new();
        for row in rows {
            let issue = self.parse_issue_from_row(
                row.id.as_ref().unwrap(),
                &row.title,
                &row.description,
                &row.status,
                &row.priority,
                row.assigned_agent_id.as_deref(),
                &row.created_at,
                &row.updated_at,
                row.resolved_at.as_deref(),
                &row.tags,
            )?;
            issues.push(issue);
        }

        debug!("Found {} issues", issues.len());
        Ok(issues)
    }

    /// Count issues
    pub async fn count(&self) -> Result<i64> {
        debug!("Counting issues");

        let row = sqlx::query!("SELECT COUNT(*) as count FROM issues")
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        let count = row.count as i64;
        debug!("Total issues count: {}", count);
        Ok(count)
    }

    /// Find issues by status
    pub async fn find_by_status(&self, status: &IssueStatus) -> Result<Vec<Issue>> {
        debug!("Finding issues by status: {:?}", status);

        let status_str = self.serialize_status(status);
        let rows = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues WHERE status = ?1 ORDER BY created_at DESC",
            status_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut issues = Vec::new();
        for row in rows {
            let issue = self.parse_issue_from_row(
                row.id.as_ref().unwrap(),
                &row.title,
                &row.description,
                &row.status,
                &row.priority,
                row.assigned_agent_id.as_deref(),
                &row.created_at,
                &row.updated_at,
                row.resolved_at.as_deref(),
                &row.tags,
            )?;
            issues.push(issue);
        }

        debug!("Found {} issues with status {:?}", issues.len(), status);
        Ok(issues)
    }

    /// Find issues by priority
    pub async fn find_by_priority(&self, priority: &IssuePriority) -> Result<Vec<Issue>> {
        debug!("Finding issues by priority: {:?}", priority);

        let priority_str = self.serialize_priority(priority);
        let rows = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues WHERE priority = ?1 ORDER BY created_at DESC",
            priority_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut issues = Vec::new();
        for row in rows {
            let issue = self.parse_issue_from_row(
                row.id.as_ref().unwrap(),
                &row.title,
                &row.description,
                &row.status,
                &row.priority,
                row.assigned_agent_id.as_deref(),
                &row.created_at,
                &row.updated_at,
                row.resolved_at.as_deref(),
                &row.tags,
            )?;
            issues.push(issue);
        }

        debug!("Found {} issues with priority {:?}", issues.len(), priority);
        Ok(issues)
    }

    /// Find issues assigned to a specific agent
    pub async fn find_by_assigned_agent(&self, agent_id: Uuid) -> Result<Vec<Issue>> {
        debug!("Finding issues assigned to agent: {}", agent_id);

        let agent_id_str = agent_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues WHERE assigned_agent_id = ?1 ORDER BY created_at DESC",
            agent_id_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut issues = Vec::new();
        for row in rows {
            let issue = self.parse_issue_from_row(
                row.id.as_ref().unwrap(),
                &row.title,
                &row.description,
                &row.status,
                &row.priority,
                row.assigned_agent_id.as_deref(),
                &row.created_at,
                &row.updated_at,
                row.resolved_at.as_deref(),
                &row.tags,
            )?;
            issues.push(issue);
        }

        debug!(
            "Found {} issues assigned to agent {}",
            issues.len(),
            agent_id
        );
        Ok(issues)
    }

    /// Check if an issue exists
    pub async fn exists(&self, id: Uuid) -> Result<bool> {
        debug!("Checking if issue exists: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!("SELECT COUNT(*) as count FROM issues WHERE id = ?1", id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        let exists = row.count > 0;
        debug!("Issue {} exists: {}", id, exists);
        Ok(exists)
    }

    /// Parse issue from database row
    #[allow(clippy::too_many_arguments)]
    fn parse_issue_from_row(
        &self,
        id: &str,
        title: &str,
        description: &str,
        status: &str,
        priority: &str,
        assigned_agent_id: Option<&str>,
        created_at: &str,
        updated_at: &str,
        resolved_at: Option<&str>,
        tags: &str,
    ) -> Result<Issue> {
        let id = Uuid::parse_str(id)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse issue ID: {}", e)))?;

        let status = self.deserialize_status(status)?;
        let priority = self.deserialize_priority(priority)?;

        let assigned_agent_id = if let Some(agent_id_str) = assigned_agent_id {
            Some(Uuid::parse_str(agent_id_str).map_err(|e| {
                Error::Internal(anyhow::anyhow!("Failed to parse assigned agent ID: {}", e))
            })?)
        } else {
            None
        };

        let created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse created_at: {}", e)))?
            .with_timezone(&Utc);

        let updated_at = DateTime::parse_from_rfc3339(updated_at)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse updated_at: {}", e)))?
            .with_timezone(&Utc);

        let resolved_at = if let Some(resolved_at_str) = resolved_at {
            Some(
                DateTime::parse_from_rfc3339(resolved_at_str)
                    .map_err(|e| {
                        Error::Internal(anyhow::anyhow!("Failed to parse resolved_at: {}", e))
                    })?
                    .with_timezone(&Utc),
            )
        } else {
            None
        };

        let tags: Vec<String> = serde_json::from_str(tags)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to deserialize tags: {}", e)))?;

        Ok(Issue {
            id,
            title: title.to_string(),
            description: description.to_string(),
            status,
            priority,
            assigned_agent_id,
            created_at,
            updated_at,
            resolved_at,
            tags,
            knowledge_links: Vec::new(), // We'll add support for these in separate tables later
            web_metadata: None,          // We'll add support for this in separate table later
        })
    }

    /// Serialize issue status to string
    fn serialize_status(&self, status: &IssueStatus) -> String {
        match status {
            IssueStatus::Open => "Open".to_string(),
            IssueStatus::InProgress => "InProgress".to_string(),
            IssueStatus::Blocked { reason } => format!("Blocked:{}", reason),
            IssueStatus::Resolved => "Resolved".to_string(),
            IssueStatus::Closed => "Closed".to_string(),
        }
    }

    /// Deserialize issue status from string
    fn deserialize_status(&self, status_str: &str) -> Result<IssueStatus> {
        match status_str {
            "Open" => Ok(IssueStatus::Open),
            "InProgress" => Ok(IssueStatus::InProgress),
            "Resolved" => Ok(IssueStatus::Resolved),
            "Closed" => Ok(IssueStatus::Closed),
            s if s.starts_with("Blocked:") => {
                let reason = s.strip_prefix("Blocked:").unwrap_or("").to_string();
                Ok(IssueStatus::Blocked { reason })
            }
            _ => Err(Error::Internal(anyhow::anyhow!(
                "Unknown issue status: {}",
                status_str
            ))),
        }
    }

    /// Serialize issue priority to string
    fn serialize_priority(&self, priority: &IssuePriority) -> String {
        match priority {
            IssuePriority::Low => "Low".to_string(),
            IssuePriority::Medium => "Medium".to_string(),
            IssuePriority::High => "High".to_string(),
            IssuePriority::Critical => "Critical".to_string(),
        }
    }

    /// Deserialize issue priority from string
    fn deserialize_priority(&self, priority_str: &str) -> Result<IssuePriority> {
        match priority_str {
            "Low" => Ok(IssuePriority::Low),
            "Medium" => Ok(IssuePriority::Medium),
            "High" => Ok(IssuePriority::High),
            "Critical" => Ok(IssuePriority::Critical),
            _ => Err(Error::Internal(anyhow::anyhow!(
                "Unknown issue priority: {}",
                priority_str
            ))),
        }
    }

    /// Find issues by tag (supports project scoping)
    pub async fn find_by_tag(&self, tag: &str) -> Result<Vec<Issue>> {
        debug!("Finding issues by tag: {}", tag);

        let rows = sqlx::query!(
            "SELECT id, title, description, status, priority, assigned_agent_id, created_at, updated_at, resolved_at, tags FROM issues ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut issues = Vec::new();
        for row in rows {
            // Parse the issue
            let issue = match self.parse_issue_from_row(
                row.id.as_ref().unwrap(),
                &row.title,
                &row.description,
                &row.status,
                &row.priority,
                row.assigned_agent_id.as_deref(),
                &row.created_at,
                &row.updated_at,
                row.resolved_at.as_deref(),
                &row.tags,
            ) {
                Ok(issue) => issue,
                Err(e) => {
                    debug!("Failed to parse issue from row: {}", e);
                    continue;
                }
            };

            // Check if the issue has the specified tag
            if issue.has_tag(tag) {
                issues.push(issue);
            }
        }

        debug!("Found {} issues with tag: {}", issues.len(), tag);
        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    include!("issue_tests.rs");
}
