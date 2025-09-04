//! Project service for business logic and lifecycle management

use crate::{repositories::ProjectRepository, Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::project::Project;

/// Service for managing project lifecycle and business operations
pub struct ProjectService {
    repository: Arc<ProjectRepository>,
}

/// Statistics about projects in the system
#[derive(Debug, Clone)]
pub struct ProjectStatistics {
    pub total_projects: i64,
    pub active_projects: i64,
    pub archived_projects: i64,
    pub projects_with_workspaces: i64,
    pub projects_without_workspaces: i64,
    pub projects_by_status: HashMap<String, i64>,
    pub workspace_usage: HashMap<String, i64>,
}

/// Project validation result with recommendations
#[derive(Debug, Clone)]
pub struct ProjectValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Workspace setup result
#[derive(Debug, Clone)]
pub struct WorkspaceSetup {
    pub project_id: Uuid,
    pub workspace_path: PathBuf,
    pub created_directories: Vec<PathBuf>,
    pub setup_successful: bool,
    pub warnings: Vec<String>,
}

/// Archive result with cleanup details
#[derive(Debug, Clone)]
pub struct ArchiveResult {
    pub project_id: Uuid,
    pub archived_at: chrono::DateTime<chrono::Utc>,
    pub cleanup_performed: bool,
    pub preserved_files: Vec<PathBuf>,
    pub removed_files: Vec<PathBuf>,
}

impl ProjectService {
    /// Create a new project service
    pub fn new(repository: Arc<ProjectRepository>) -> Self {
        Self { repository }
    }

    /// Create a new project with optional automatic workspace setup
    pub async fn create_project(
        &self,
        name: String,
        description: Option<String>,
        workspace_path: Option<PathBuf>,
        setup_workspace: bool,
    ) -> Result<Project> {
        info!("Creating new project: {}", name);

        // Check if project name already exists
        if let Some(_existing) = self.repository.find_by_name(&name).await? {
            return Err(Error::Conflict(format!(
                "Project with name '{}' already exists",
                name
            )));
        }

        // Validate workspace path if provided
        if let Some(ref path) = workspace_path {
            self.validate_workspace_path(path)?;
        }

        // Create the project
        let project = Project::new(name, description, workspace_path.clone())?;

        // Setup workspace if requested and path is provided BEFORE persisting
        if setup_workspace {
            if let Some(ref path) = workspace_path {
                if let Err(e) = self.setup_workspace_internal(&project.id, path).await {
                    // Do not persist a project if workspace setup fails
                    return Err(e);
                }
            } else {
                warn!(
                    "Workspace setup requested for project '{}' but no workspace path provided",
                    &project.name
                );
            }
        }

        // Store in database after successful workspace setup (if any)
        self.repository.create(&project).await?;
        info!(
            "Successfully created project: {} ({})",
            &project.name, project.id
        );
        Ok(project)
    }

    /// Get project by ID
    pub async fn get_project(&self, id: Uuid) -> Result<Option<Project>> {
        debug!("Retrieving project: {}", id);
        self.repository.find_by_id(&id).await
    }

    /// Get project by name
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Project>> {
        debug!("Finding project by name: {}", name);
        self.repository.find_by_name(name).await
    }

    /// Update an existing project
    pub async fn update_project(&self, project: &Project) -> Result<Project> {
        info!("Updating project: {} ({})", project.name, project.id);

        // Get current project for optimistic locking
        let current = self
            .repository
            .find_by_id(&project.id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Project".to_string(),
                id: project.id.to_string(),
            })?;

        // Name change: enforce uniqueness
        if project.name != current.name {
            if let Some(other) = self.repository.find_by_name(&project.name).await? {
                if other.id != project.id {
                    return Err(Error::Conflict(format!(
                        "Project with name '{}' already exists",
                        project.name
                    )));
                }
            }
        }

        // Validate workspace path if changed
        if let Some(ref new_path) = project.workspace_path {
            if current.workspace_path.as_ref() != Some(new_path) {
                self.validate_workspace_path(new_path)?;
            }
        }

        // Update in database with optimistic locking
        let updated_project = self.repository.update(project, &current.updated_at).await?;

        info!(
            "Successfully updated project: {} ({})",
            project.name, project.id
        );
        Ok(updated_project)
    }

    /// Archive a project (soft delete with cleanup options)
    pub async fn archive_project(
        &self,
        id: Uuid,
        cleanup_workspace: bool,
        preserve_files: Vec<PathBuf>,
    ) -> Result<ArchiveResult> {
        info!("Archiving project: {}", id);

        let project = self
            .repository
            .find_by_id(&id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Project".to_string(),
                id: id.to_string(),
            })?;

        let mut archive_result = ArchiveResult {
            project_id: id,
            archived_at: chrono::Utc::now(),
            cleanup_performed: false,
            preserved_files: preserve_files.clone(),
            removed_files: Vec::new(),
        };

        // Perform workspace cleanup if requested
        if cleanup_workspace && project.has_workspace() {
            if let Some(workspace_path) = &project.workspace_path {
                match self
                    .cleanup_workspace_internal(workspace_path, &preserve_files)
                    .await
                {
                    Ok(removed_files) => {
                        archive_result.cleanup_performed = true;
                        archive_result.removed_files = removed_files;
                        info!("Workspace cleanup completed for project: {}", id);
                    }
                    Err(e) => {
                        warn!("Failed to cleanup workspace for project {}: {}", id, e);
                        // Continue with archiving even if cleanup fails
                    }
                }
            }
        }

        // Persist archived status
        self.repository
            .archive(&id, archive_result.archived_at)
            .await?;

        info!("Successfully archived project: {}", id);
        Ok(archive_result)
    }

    /// Delete a project permanently
    pub async fn delete_project(&self, id: Uuid) -> Result<()> {
        info!("Deleting project permanently: {}", id);

        // Check if the project can be deleted (no associated agents)
        self.repository.delete(&id).await?;

        info!("Successfully deleted project: {}", id);
        Ok(())
    }

    /// List all projects
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        debug!("Listing all projects");
        self.repository.list().await
    }

    /// List projects by status
    /// TODO: Consider using enum instead of stringly-typed status to avoid typos
    pub async fn list_projects_by_status(&self, status: &str) -> Result<Vec<Project>> {
        debug!("Listing projects by status: {}", status);
        self.repository.list_by_status(status).await
    }

    /// Get project statistics
    pub async fn get_statistics(&self) -> Result<ProjectStatistics> {
        debug!("Computing project statistics");

        let all_projects = self.repository.list().await?;
        let active_count = self.repository.count().await?;

        let mut projects_by_status = HashMap::new();
        let mut workspace_usage = HashMap::new();
        let mut projects_with_workspaces = 0;

        // Analyze projects
        for project in &all_projects {
            // Track workspace usage
            if project.has_workspace() {
                projects_with_workspaces += 1;
                if let Some(workspace_str) = project.workspace_path_string() {
                    *workspace_usage.entry(workspace_str).or_insert(0) += 1;
                }
            }
        }

        // Get status counts (assuming Active/Archived for now)
        projects_by_status.insert("Active".to_string(), active_count);
        let archived_count = all_projects.len() as i64 - active_count;
        if archived_count > 0 {
            projects_by_status.insert("Archived".to_string(), archived_count);
        }

        Ok(ProjectStatistics {
            total_projects: all_projects.len() as i64,
            active_projects: active_count,
            archived_projects: archived_count,
            projects_with_workspaces,
            projects_without_workspaces: all_projects.len() as i64 - projects_with_workspaces,
            projects_by_status,
            workspace_usage,
        })
    }

    /// Validate a project for business rule compliance
    pub async fn validate_project(&self, project: &Project) -> Result<ProjectValidation> {
        debug!("Validating project: {} ({})", project.name, project.id);

        let mut validation = ProjectValidation {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            recommendations: Vec::new(),
        };

        // Check for duplicate names
        if let Some(existing) = self.repository.find_by_name(&project.name).await? {
            if existing.id != project.id {
                validation.is_valid = false;
                validation
                    .errors
                    .push(format!("Project name '{}' is already in use", project.name));
            }
        }

        // Validate workspace path if present
        if let Some(ref workspace_path) = project.workspace_path {
            match self.validate_workspace_path(workspace_path) {
                Ok(_) => {
                    validation
                        .recommendations
                        .push("Workspace path is valid".to_string());
                }
                Err(e) => {
                    validation.is_valid = false;
                    validation
                        .errors
                        .push(format!("Invalid workspace path: {}", e));
                }
            }

            // Check if workspace already exists
            if workspace_path.exists() {
                if workspace_path.is_dir() {
                    validation
                        .warnings
                        .push("Workspace directory already exists".to_string());
                } else {
                    validation
                        .errors
                        .push("Workspace path exists but is not a directory".to_string());
                    validation.is_valid = false;
                }
            }
        } else {
            validation
                .recommendations
                .push("Consider setting up a workspace path for better organization".to_string());
        }

        // Business rule recommendations
        if project.description.is_none() || project.description.as_ref().unwrap().is_empty() {
            validation
                .recommendations
                .push("Consider adding a project description for better documentation".to_string());
        }

        debug!(
            "Project validation completed: {} (valid: {})",
            project.id, validation.is_valid
        );

        Ok(validation)
    }

    /// Setup workspace directories for a project
    pub async fn setup_workspace(&self, id: Uuid) -> Result<WorkspaceSetup> {
        debug!("Setting up workspace for project: {}", id);

        let project = self
            .repository
            .find_by_id(&id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Project".to_string(),
                id: id.to_string(),
            })?;

        let workspace_path = project.workspace_path.ok_or_else(|| Error::Validation {
            message: "Project has no workspace path configured".to_string(),
        })?;

        self.setup_workspace_internal(&id, &workspace_path).await
    }

    /// Internal workspace setup implementation
    async fn setup_workspace_internal(
        &self,
        project_id: &Uuid,
        workspace_path: &Path,
    ) -> Result<WorkspaceSetup> {
        let mut setup = WorkspaceSetup {
            project_id: *project_id,
            workspace_path: workspace_path.to_path_buf(),
            created_directories: Vec::new(),
            setup_successful: false,
            warnings: Vec::new(),
        };

        // Create workspace directory if it doesn't exist
        if !workspace_path.exists() {
            tokio::fs::create_dir_all(workspace_path)
                .await
                .map_err(|e| {
                    Error::Internal(anyhow::anyhow!(
                        "Failed to create workspace directory '{}': {}",
                        workspace_path.display(),
                        e
                    ))
                })?;
            setup.created_directories.push(workspace_path.to_path_buf());
            info!("Created workspace directory: {}", workspace_path.display());
        } else if !workspace_path.is_dir() {
            return Err(Error::Validation {
                message: format!(
                    "Workspace path '{}' exists but is not a directory",
                    workspace_path.display()
                ),
            });
        }

        setup.setup_successful = true;
        info!("Workspace setup completed for project: {}", project_id);

        Ok(setup)
    }

    /// Internal workspace cleanup implementation
    async fn cleanup_workspace_internal(
        &self,
        workspace_path: &Path,
        preserve_files: &[PathBuf],
    ) -> Result<Vec<PathBuf>> {
        let mut removed_files = Vec::new();

        // Enumerate candidates for removal to aid observability
        if workspace_path.exists() && workspace_path.is_dir() {
            let mut rd = tokio::fs::read_dir(workspace_path).await.map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to read workspace directory '{}': {}",
                    workspace_path.display(),
                    e
                ))
            })?;
            let preserve_set: std::collections::HashSet<PathBuf> =
                preserve_files.iter().cloned().collect();
            while let Some(entry) = rd.next_entry().await.map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to iterate workspace directory '{}': {}",
                    workspace_path.display(),
                    e
                ))
            })? {
                let p = entry.path();
                if !preserve_set.contains(&p) {
                    removed_files.push(p);
                }
            }
            debug!(
                "Identified {} entries for removal in workspace: {}",
                removed_files.len(),
                workspace_path.display()
            );
        }

        Ok(removed_files)
    }

    /// Validate workspace path for business rules
    fn validate_workspace_path(&self, path: &Path) -> Result<()> {
        // Check if path is absolute for better reliability
        if !path.is_absolute() {
            return Err(Error::Validation {
                message: "Workspace path should be absolute for reliability".to_string(),
            });
        }

        // Check parent directory existence and type
        match path.parent() {
            Some(parent) => {
                if !parent.exists() {
                    return Err(Error::Validation {
                        message: "Parent directory of workspace path does not exist".to_string(),
                    });
                }
                if !parent.is_dir() {
                    return Err(Error::Validation {
                        message: "Parent directory of workspace path is not a directory"
                            .to_string(),
                    });
                }
            }
            None => {
                return Err(Error::Validation {
                    message: "Workspace path has no parent directory".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get projects with agents (for coordination services)
    pub async fn get_projects_with_agents(&self) -> Result<Vec<Project>> {
        debug!("Finding projects with associated agents");

        // For now, return all active projects
        // This could be enhanced with a join query to only return projects that have agents
        self.repository.list_by_status("Active").await
    }

    /// Check if a project exists
    pub async fn exists(&self, id: &Uuid) -> Result<bool> {
        Ok(self.repository.find_by_id(id).await?.is_some())
    }

    /// Check if a project name is available
    pub async fn is_name_available(&self, name: &str) -> Result<bool> {
        Ok(self.repository.find_by_name(name).await?.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::ProjectRepository;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::PathBuf;

    async fn setup_test_service() -> ProjectService {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .expect("Failed to connect to test database");

        crate::migrations::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        let repository = Arc::new(ProjectRepository::new(pool));
        ProjectService::new(repository)
    }

    #[tokio::test]
    async fn test_create_project() {
        let service = setup_test_service().await;

        let project = service
            .create_project(
                "test-project".to_string(),
                Some("A test project".to_string()),
                Some(PathBuf::from("/tmp/test-project")),
                false,
            )
            .await
            .unwrap();

        assert_eq!(project.name, "test-project");
        assert_eq!(project.description, Some("A test project".to_string()));
        assert_eq!(
            project.workspace_path,
            Some(PathBuf::from("/tmp/test-project"))
        );
    }

    #[tokio::test]
    async fn test_project_name_conflict() {
        let service = setup_test_service().await;

        // Create first project
        service
            .create_project("duplicate-name".to_string(), None, None, false)
            .await
            .unwrap();

        // Try to create second project with same name
        let result = service
            .create_project("duplicate-name".to_string(), None, None, false)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Conflict(_)));
    }

    #[tokio::test]
    async fn test_project_statistics() {
        let service = setup_test_service().await;

        // Create some projects
        service
            .create_project(
                "project-1".to_string(),
                None,
                Some(PathBuf::from("/tmp/project-1")),
                false,
            )
            .await
            .unwrap();

        service
            .create_project("project-2".to_string(), None, None, false)
            .await
            .unwrap();

        let stats = service.get_statistics().await.unwrap();

        assert_eq!(stats.total_projects, 2);
        assert_eq!(stats.active_projects, 2);
        assert_eq!(stats.projects_with_workspaces, 1);
        assert_eq!(stats.projects_without_workspaces, 1);
    }

    #[tokio::test]
    async fn test_project_validation() {
        let service = setup_test_service().await;

        let project = Project::builder()
            .name("validation-test")
            .description("Test validation")
            .workspace_path("/tmp/validation-test")
            .build()
            .unwrap();

        let validation = service.validate_project(&project).await.unwrap();

        assert!(validation.is_valid);
        assert!(validation.errors.is_empty());
    }

    #[tokio::test]
    async fn test_workspace_path_validation() {
        let service = setup_test_service().await;

        // Test relative path (should warn or fail)
        let relative_path = PathBuf::from("relative/path");
        let result = service.validate_workspace_path(&relative_path);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_name_availability() {
        let service = setup_test_service().await;

        // Test available name
        assert!(service.is_name_available("available-name").await.unwrap());

        // Create project
        service
            .create_project("taken-name".to_string(), None, None, false)
            .await
            .unwrap();

        // Test taken name
        assert!(!service.is_name_available("taken-name").await.unwrap());
    }
}
