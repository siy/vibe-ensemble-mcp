//! Project repository implementation

use crate::{Error, Result};
use anyhow;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};
use std::path::PathBuf;
use tracing::{debug, info};
use uuid::Uuid;
use vibe_ensemble_core::project::Project;

/// Repository for project entities
pub struct ProjectRepository {
    pool: Pool<Sqlite>,
}

impl ProjectRepository {
    /// Create a new project repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new project
    pub async fn create(&self, project: &Project) -> Result<()> {
        debug!("Creating project: {} ({})", project.name, project.id);

        let project_id_str = project.id.to_string();
        let created_at_str = project.created_at.to_rfc3339();
        let updated_at_str = project.updated_at.to_rfc3339();
        let workspace_path_str = project.workspace_path_string();

        sqlx::query!(
            r#"
            INSERT INTO projects (id, name, description, working_directory, git_repository, created_at, updated_at, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            project_id_str,
            project.name,
            project.description,
            workspace_path_str,
            None::<String>, // git_repository will be added later if needed
            created_at_str,
            updated_at_str,
            "Active"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.code() == Some(std::borrow::Cow::Borrowed("2067")) => {
                // SQLite unique constraint violation
                Error::ConstraintViolation(format!(
                    "Project name '{}' already exists",
                    project.name
                ))
            }
            _ => Error::Database(e),
        })?;

        info!(
            "Successfully created project: {} ({})",
            project.name, project.id
        );
        Ok(())
    }

    /// Find a project by ID
    pub async fn find_by_id(&self, id: &Uuid) -> Result<Option<Project>> {
        debug!("Finding project by ID: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, name, description, working_directory, git_repository, created_at, updated_at, status FROM projects WHERE id = ?1",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let project = self.parse_project_from_row(
                    row.id.as_ref().unwrap(),
                    &row.name,
                    row.description.as_deref(),
                    row.working_directory.as_deref(),
                    row.git_repository.as_deref(),
                    &row.created_at,
                    &row.updated_at,
                    &row.status,
                )?;
                Ok(Some(project))
            }
            None => Ok(None),
        }
    }

    /// Find a project by name
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Project>> {
        debug!("Finding project by name: {}", name);

        let row = sqlx::query!(
            "SELECT id, name, description, working_directory, git_repository, created_at, updated_at, status FROM projects WHERE name = ?1",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let project = self.parse_project_from_row(
                    row.id.as_ref().unwrap(),
                    &row.name,
                    row.description.as_deref(),
                    row.working_directory.as_deref(),
                    row.git_repository.as_deref(),
                    &row.created_at,
                    &row.updated_at,
                    &row.status,
                )?;
                Ok(Some(project))
            }
            None => Ok(None),
        }
    }

    /// Update a project with optimistic locking
    pub async fn update(
        &self,
        project: &Project,
        old_updated_at: &DateTime<Utc>,
    ) -> Result<Project> {
        debug!("Updating project: {} ({})", project.name, project.id);

        let project_id_str = project.id.to_string();
        let old_updated_at_str = old_updated_at.to_rfc3339();
        let workspace_path_str = project.workspace_path_string();

        let result = sqlx::query!(
            r#"
            UPDATE projects 
            SET name = ?1, description = ?2, working_directory = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            WHERE id = ?4 AND updated_at = ?5
            RETURNING id, name, description, working_directory, git_repository, created_at, updated_at, status
            "#,
            project.name,
            project.description,
            workspace_path_str,
            project_id_str,
            old_updated_at_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err)
                if db_err.code() == Some(std::borrow::Cow::Borrowed("2067")) =>
            {
                // SQLite unique constraint violation
                Error::ConstraintViolation(format!(
                    "Project name '{}' already exists",
                    project.name
                ))
            }
            _ => Error::Database(e),
        })?;

        match result {
            Some(row) => {
                let updated_project = self.parse_project_from_row(
                    row.id.as_ref().unwrap(),
                    &row.name,
                    row.description.as_deref(),
                    row.working_directory.as_deref(),
                    row.git_repository.as_deref(),
                    &row.created_at,
                    &row.updated_at,
                    &row.status,
                )?;

                info!(
                    "Successfully updated project: {} ({})",
                    project.name, project.id
                );
                Ok(updated_project)
            }
            None => Err(Error::Conflict(format!(
                "Project {} was modified by another process. Please reload and try again.",
                project.id
            ))),
        }
    }

    /// Delete a project
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        debug!("Deleting project: {}", id);

        let id_str = id.to_string();

        // First, check if any agents are associated with this project
        let agent_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM agents WHERE project_id = ?1",
            id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        if agent_count.count > 0 {
            return Err(Error::ConstraintViolation(format!(
                "Cannot delete project {} - it has {} associated agents. Remove agents first.",
                id, agent_count.count
            )));
        }

        let result = sqlx::query!("DELETE FROM projects WHERE id = ?1", id_str)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::NotFound {
                entity: "Project".to_string(),
                id: id.to_string(),
            });
        }

        info!("Successfully deleted project: {}", id);
        Ok(())
    }

    /// List all projects
    pub async fn list(&self) -> Result<Vec<Project>> {
        debug!("Listing all projects");

        let rows = sqlx::query!(
            "SELECT id, name, description, working_directory, git_repository, created_at, updated_at, status FROM projects ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut projects = Vec::new();
        for row in rows {
            let project = self.parse_project_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                row.description.as_deref(),
                row.working_directory.as_deref(),
                row.git_repository.as_deref(),
                &row.created_at,
                &row.updated_at,
                &row.status,
            )?;
            projects.push(project);
        }

        debug!("Found {} projects", projects.len());
        Ok(projects)
    }

    /// List projects by status
    pub async fn list_by_status(&self, status: &str) -> Result<Vec<Project>> {
        debug!("Listing projects by status: {}", status);

        let rows = sqlx::query!(
            "SELECT id, name, description, working_directory, git_repository, created_at, updated_at, status FROM projects WHERE status = ?1 ORDER BY created_at DESC",
            status
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut projects = Vec::new();
        for row in rows {
            let project = self.parse_project_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                row.description.as_deref(),
                row.working_directory.as_deref(),
                row.git_repository.as_deref(),
                &row.created_at,
                &row.updated_at,
                &row.status,
            )?;
            projects.push(project);
        }

        debug!("Found {} projects with status: {}", projects.len(), status);
        Ok(projects)
    }

    /// Count total number of active projects
    pub async fn count(&self) -> Result<i64> {
        debug!("Counting active projects");
        let row =
            sqlx::query!("SELECT COUNT(*) as projects_count FROM projects WHERE status = 'Active'")
                .fetch_one(&self.pool)
                .await
                .map_err(Error::Database)?;
        Ok(row.projects_count as i64)
    }

    /// Parse a project from database row data
    #[allow(clippy::too_many_arguments)]
    fn parse_project_from_row(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        working_directory: Option<&str>,
        _git_repository: Option<&str>, // Reserved for future use
        created_at: &str,
        updated_at: &str,
        _status: &str, // Status field for future enhancement
    ) -> Result<Project> {
        // Validate name is not empty
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Project name cannot be empty".to_string(),
            });
        }

        let parsed_id = Uuid::parse_str(id).map_err(|e| {
            Error::Internal(anyhow::anyhow!("Invalid project UUID '{}': {}", id, e))
        })?;

        let parsed_created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to parse created_at '{}': {}",
                    created_at,
                    e
                ))
            })?
            .with_timezone(&Utc);

        let parsed_updated_at = DateTime::parse_from_rfc3339(updated_at)
            .map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to parse updated_at '{}': {}",
                    updated_at,
                    e
                ))
            })?
            .with_timezone(&Utc);

        let workspace_path = working_directory.map(PathBuf::from);

        Ok(Project {
            id: parsed_id,
            name: name.trim().to_string(),
            description: description
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            created_at: parsed_created_at,
            updated_at: parsed_updated_at,
            workspace_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use std::path::PathBuf;

    async fn setup_test_db() -> ProjectRepository {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("Failed to connect to test database");

        // Run migrations using proper module
        crate::migrations::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        ProjectRepository::new(pool)
    }

    #[tokio::test]
    async fn test_project_crud_operations() {
        let project_repo = setup_test_db().await;

        // Test create
        let project = Project::builder()
            .name("test-project")
            .description("A test project for CRUD operations")
            .workspace_path("/tmp/test-project")
            .build()
            .unwrap();

        let project_id = project.id;

        project_repo.create(&project).await.unwrap();

        // Test find_by_id
        let found_project = project_repo.find_by_id(&project_id).await.unwrap();
        assert!(found_project.is_some());
        let found_project = found_project.unwrap();
        assert_eq!(found_project.name, "test-project");
        assert_eq!(
            found_project.description,
            Some("A test project for CRUD operations".to_string())
        );
        assert_eq!(
            found_project.workspace_path,
            Some(PathBuf::from("/tmp/test-project"))
        );

        // Test find_by_name
        let found_by_name = project_repo.find_by_name("test-project").await.unwrap();
        assert!(found_by_name.is_some());
        assert_eq!(found_by_name.unwrap().id, project_id);

        // Test update
        let mut updated_project = found_project.clone();
        updated_project
            .set_name("updated-test-project".to_string())
            .unwrap();
        updated_project
            .set_description(Some("Updated description".to_string()))
            .unwrap();

        let old_updated_at = found_project.updated_at;
        let result_project = project_repo
            .update(&updated_project, &old_updated_at)
            .await
            .unwrap();
        assert_eq!(result_project.name, "updated-test-project");
        assert_eq!(
            result_project.description,
            Some("Updated description".to_string())
        );

        let found_updated = project_repo.find_by_id(&project_id).await.unwrap().unwrap();
        assert_eq!(found_updated.name, "updated-test-project");
        assert_eq!(
            found_updated.description,
            Some("Updated description".to_string())
        );

        // Test list
        let all_projects = project_repo.list().await.unwrap();
        assert_eq!(all_projects.len(), 1);
        assert_eq!(all_projects[0].id, project_id);

        // Test list_by_status
        let active_projects = project_repo.list_by_status("Active").await.unwrap();
        assert_eq!(active_projects.len(), 1);

        let inactive_projects = project_repo.list_by_status("Inactive").await.unwrap();
        assert_eq!(inactive_projects.len(), 0);

        // Test delete
        project_repo.delete(&project_id).await.unwrap();

        let deleted_project = project_repo.find_by_id(&project_id).await.unwrap();
        assert!(deleted_project.is_none());
    }

    #[tokio::test]
    async fn test_project_minimal_creation() {
        let project_repo = setup_test_db().await;

        let project = Project::builder().name("minimal-project").build().unwrap();

        let project_id = project.id;

        project_repo.create(&project).await.unwrap();

        let found_project = project_repo.find_by_id(&project_id).await.unwrap().unwrap();
        assert_eq!(found_project.name, "minimal-project");
        assert_eq!(found_project.description, None);
        assert_eq!(found_project.workspace_path, None);
    }

    #[tokio::test]
    async fn test_project_unique_name_constraint() {
        let project_repo = setup_test_db().await;

        let project1 = Project::builder().name("duplicate-name").build().unwrap();

        let project2 = Project::builder().name("duplicate-name").build().unwrap();

        // First creation should succeed
        project_repo.create(&project1).await.unwrap();

        // Second creation should fail due to unique name constraint
        let result = project_repo.create(&project2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_project_not_found_operations() {
        let project_repo = setup_test_db().await;

        let non_existent_id = Uuid::new_v4();

        // Test find non-existent project
        let result = project_repo.find_by_id(&non_existent_id).await.unwrap();
        assert!(result.is_none());

        // Test find by non-existent name
        let result = project_repo.find_by_name("non-existent").await.unwrap();
        assert!(result.is_none());

        // Test update non-existent project
        let fake_project = Project::builder().name("fake-project").build().unwrap();

        let mut fake_project_with_id = fake_project;
        fake_project_with_id.id = non_existent_id;

        let fake_old_updated_at = Utc::now();
        let result = project_repo
            .update(&fake_project_with_id, &fake_old_updated_at)
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Conflict(_)));

        // Test delete non-existent project
        let result = project_repo.delete(&non_existent_id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_project_agents_constraint() {
        let project_repo = setup_test_db().await;

        // Create a project
        let project = Project::builder()
            .name("project-with-agents")
            .build()
            .unwrap();

        let project_id = project.id;
        project_repo.create(&project).await.unwrap();

        // Manually insert an agent with project association to test constraint
        // (In real usage, this would be done through AgentRepository)
        let agent_id = Uuid::new_v4();
        let project_id_str = project_id.to_string();
        let agent_id_str = agent_id.to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO agents (id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen, project_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#
        )
        .bind(&agent_id_str)
        .bind("test-agent")
        .bind("Worker")
        .bind("[]")
        .bind(r#"{"active": true}"#)
        .bind(r#"{"protocol": "test"}"#)
        .bind(&now)
        .bind(&now)
        .bind(&project_id_str)
        .execute(&project_repo.pool)
        .await
        .unwrap();

        // Now try to delete the project - should fail due to constraint
        let result = project_repo.delete(&project_id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::ConstraintViolation(_)));

        // Clean up the agent first
        sqlx::query!("DELETE FROM agents WHERE id = ?1", agent_id_str)
            .execute(&project_repo.pool)
            .await
            .unwrap();

        // Now deletion should succeed
        project_repo.delete(&project_id).await.unwrap();
    }
}
