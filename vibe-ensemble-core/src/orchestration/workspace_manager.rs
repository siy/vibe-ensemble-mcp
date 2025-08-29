//! Persistent workspace lifecycle management
//!
//! This module provides functionality for creating, managing, and persisting
//! Claude Code agent workspaces. Workspaces are isolated environments where
//! agents can operate with specific configurations and project contexts.
//!
//! # Workspace Structure
//!
//! ```text
//! workspaces/
//! +-- workspace-name/
//!     +-- workspace.json         # Workspace configuration and metadata
//!     +-- .claude/               # Claude Code configuration directory
//!     |   +-- agents/            # Agent-specific configurations
//!     |       +-- agent.md       # Generated agent configuration
//!     +-- project/               # Project directory for agent work
//!         +-- ...                # Project files
//! ```

use crate::orchestration::models::{FilesystemTemplate, WorkspaceConfiguration};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

/// Configuration for workspace creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Template variables for agent configuration
    pub variables: HashMap<String, String>,
    /// Whether to reuse existing workspace if it exists
    pub reuse_existing: bool,
    /// Custom project directory structure to create
    pub project_structure: Vec<ProjectItem>,
    /// Additional environment variables for the workspace
    pub environment: HashMap<String, String>,
    /// Git repository to clone into the project directory
    pub git_repository: Option<GitConfig>,
}

/// Project item to create in workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectItem {
    /// Path relative to project directory
    pub path: String,
    /// Item type
    pub item_type: ProjectItemType,
    /// Content for files (ignored for directories)
    pub content: Option<String>,
}

/// Type of project item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectItemType {
    Directory,
    File,
}

/// Git repository configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Repository URL
    pub url: String,
    /// Branch to checkout
    pub branch: Option<String>,
    /// Specific commit or tag
    pub ref_name: Option<String>,
}

/// Git worktree configuration for parallel development
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorktreeConfig {
    /// Main repository path (where worktrees are created from)
    pub main_repo_path: PathBuf,
    /// Branch name for the worktree
    pub branch_name: String,
    /// Whether to create a new branch or use existing
    pub create_branch: bool,
    /// Base branch to branch from (if creating new branch)
    pub base_branch: Option<String>,
}

/// Git worktree information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorktreeInfo {
    /// Worktree path
    pub path: PathBuf,
    /// Branch name
    pub branch: String,
    /// Associated agent ID
    pub agent_id: Option<String>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last used timestamp  
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    /// Whether worktree is currently active
    pub is_active: bool,
}

/// Registry of active workspaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    /// Map of workspace name to configuration
    pub workspaces: HashMap<String, WorkspaceConfiguration>,
    /// Map of worktree path to worktree info
    pub worktrees: HashMap<String, GitWorktreeInfo>,
    /// Last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            reuse_existing: true,
            project_structure: Vec::new(),
            environment: HashMap::new(),
            git_repository: None,
        }
    }
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self {
            workspaces: HashMap::new(),
            worktrees: HashMap::new(),
            updated_at: chrono::Utc::now(),
        }
    }
}

/// Workspace manager for creating and managing agent workspaces
#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    /// Root directory for all workspaces
    pub workspaces_directory: PathBuf,
    /// Registry file path
    registry_path: PathBuf,
}

impl WorkspaceManager {
    /// Create a new workspace manager
    pub fn new<P: AsRef<Path>>(workspaces_directory: P) -> Self {
        let workspaces_dir = workspaces_directory.as_ref().to_path_buf();
        let registry_path = workspaces_dir.join("registry.json");

        Self {
            workspaces_directory: workspaces_dir,
            registry_path,
        }
    }

    /// Create a new workspace from a template
    pub async fn create_workspace(
        &self,
        name: &str,
        template: &FilesystemTemplate,
        config: &WorkspaceConfig,
    ) -> Result<WorkspaceConfiguration> {
        // Validate workspace name
        self.validate_workspace_name(name)?;

        let workspace_path = self.workspaces_directory.join(name);

        // Check if workspace exists and handle reuse
        if workspace_path.exists() {
            if config.reuse_existing {
                return self.load_workspace_config(name).await;
            } else {
                return Err(Error::AlreadyExists {
                    resource: "workspace".to_string(),
                    id: name.to_string(),
                });
            }
        }

        // Create workspace directory structure
        fs::create_dir_all(&workspace_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to create workspace directory: {}", e),
            })?;

        let project_path = workspace_path.join("project");
        fs::create_dir_all(&project_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to create project directory: {}", e),
            })?;

        let agent_config_dir = workspace_path.join(".claude").join("agents");
        fs::create_dir_all(&agent_config_dir)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to create agent config directory: {}", e),
            })?;

        // Create workspace configuration
        let workspace_config = WorkspaceConfiguration::new(
            name.to_string(),
            template,
            workspace_path.clone(),
            config.variables.clone(),
        );

        // Save workspace configuration
        let config_path = workspace_path.join("workspace.json");
        let config_json = serde_json::to_string_pretty(&workspace_config).map_err(|e| {
            Error::Serialization(format!("Failed to serialize workspace config: {}", e))
        })?;

        fs::write(&config_path, config_json)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to write workspace config: {}", e),
            })?;

        // Create project structure
        self.create_project_structure(&project_path, &config.project_structure)
            .await?;

        // Clone git repository if specified
        if let Some(git_config) = &config.git_repository {
            self.clone_git_repository(&project_path, git_config).await?;
        }

        // Update registry
        self.add_to_registry(&workspace_config).await?;

        Ok(workspace_config)
    }

    /// Load an existing workspace configuration
    pub async fn load_workspace_config(&self, name: &str) -> Result<WorkspaceConfiguration> {
        let workspace_path = self.workspaces_directory.join(name);
        let config_path = workspace_path.join("workspace.json");

        if !config_path.exists() {
            return Err(Error::NotFound {
                entity_type: "workspace configuration".to_string(),
                id: name.to_string(),
            });
        }

        let config_content = fs::read_to_string(&config_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read workspace config: {}", e),
            })?;

        let mut config: WorkspaceConfiguration =
            serde_json::from_str(&config_content).map_err(|e| Error::Parsing {
                message: format!("Failed to parse workspace config: {}", e),
            })?;

        // Update last used timestamp
        config.mark_used();

        // Save updated config
        let updated_json = serde_json::to_string_pretty(&config).map_err(|e| {
            Error::Serialization(format!("Failed to serialize updated config: {}", e))
        })?;

        fs::write(&config_path, updated_json)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to update workspace config: {}", e),
            })?;

        // Update registry
        self.update_in_registry(&config).await?;

        Ok(config)
    }

    /// List all available workspaces
    pub async fn list_workspaces(&self) -> Result<Vec<String>> {
        if !self.workspaces_directory.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&self.workspaces_directory)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read workspaces directory: {}", e),
            })?;

        let mut workspaces = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io {
            message: format!("Failed to read directory entry: {}", e),
        })? {
            let path = entry.path();
            if path.is_dir() && path.file_name().unwrap() != "registry.json" {
                let config_path = path.join("workspace.json");
                if config_path.exists() {
                    if let Some(name) = path.file_name() {
                        workspaces.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        workspaces.sort();
        Ok(workspaces)
    }

    /// Check if a workspace exists
    pub async fn workspace_exists(&self, name: &str) -> Result<bool> {
        let workspace_path = self.workspaces_directory.join(name);
        let config_path = workspace_path.join("workspace.json");
        Ok(workspace_path.is_dir() && config_path.exists())
    }

    /// Delete a workspace
    pub async fn delete_workspace(&self, name: &str) -> Result<()> {
        let workspace_path = self.workspaces_directory.join(name);

        if !workspace_path.exists() {
            return Err(Error::NotFound {
                entity_type: "workspace".to_string(),
                id: name.to_string(),
            });
        }

        // Remove from registry first
        self.remove_from_registry(name).await?;

        // Delete workspace directory
        fs::remove_dir_all(&workspace_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to delete workspace directory: {}", e),
            })?;

        Ok(())
    }

    /// Get workspace statistics
    pub async fn get_workspace_stats(&self, name: &str) -> Result<WorkspaceStats> {
        let workspace_config = self.load_workspace_config(name).await?;
        let workspace_path = &workspace_config.workspace_path;

        let mut stats = WorkspaceStats {
            name: name.to_string(),
            created_at: workspace_config.created_at,
            last_used_at: workspace_config.last_used_at,
            is_active: workspace_config.is_active,
            template_name: workspace_config.template_name.clone(),
            template_version: workspace_config.template_version.clone(),
            file_count: 0,
            directory_count: 0,
            total_size_bytes: 0,
        };

        // Calculate workspace size and file counts
        self.calculate_directory_stats(workspace_path, &mut stats)
            .await?;

        Ok(stats)
    }

    /// Create a git worktree for parallel agent development
    pub async fn create_worktree(
        &self,
        name: &str,
        config: &GitWorktreeConfig,
        agent_id: Option<String>,
    ) -> Result<GitWorktreeInfo> {
        // Validate worktree name
        self.validate_workspace_name(name)?;

        // Canonicalize repo path and build a manager-scoped worktree path:
        let repo_path = tokio::fs::canonicalize(&config.main_repo_path)
            .await
            .map_err(|e| Error::Validation {
                message: format!("main_repo_path must exist: {e}"),
            })?;
        let repo_name = repo_path
            .file_name()
            .ok_or_else(|| Error::Validation {
                message: "main_repo_path must not be filesystem root".into(),
            })?
            .to_string_lossy()
            .to_string();
        let worktree_path = self
            .workspaces_directory
            .join("worktrees")
            .join(&repo_name)
            .join(name);

        // Create parent directories
        if let Some(parent) = worktree_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Execution {
                    message: format!("Failed to create worktree parent directories: {e}"),
                })?;
        }

        // Check if worktree already exists
        if worktree_path.exists() {
            return Err(Error::AlreadyExists {
                resource: "worktree".to_string(),
                id: worktree_path.display().to_string(),
            });
        }

        // Create git worktree
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("worktree")
            .arg("add")
            .current_dir(&config.main_repo_path);

        if config.create_branch {
            cmd.arg("-b").arg(&config.branch_name);
        }

        cmd.arg(&worktree_path);

        if config.create_branch {
            if let Some(base_branch) = &config.base_branch {
                cmd.arg(base_branch);
            } else {
                cmd.arg("main"); // Default base branch
            }
        } else {
            cmd.arg(&config.branch_name);
        }

        use std::time::Duration;
        let output = tokio::time::timeout(Duration::from_secs(30), cmd.output())
            .await
            .map_err(|_| Error::Execution {
                message: "Timed out executing git worktree add".into(),
            })?
            .map_err(|e| Error::Execution {
                message: format!("Failed to execute git worktree add: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Execution {
                message: format!("Git worktree add failed: {}", stderr),
            });
        }

        // Create worktree info
        let worktree_info = GitWorktreeInfo {
            path: worktree_path,
            branch: config.branch_name.clone(),
            agent_id: agent_id.clone(),
            created_at: chrono::Utc::now(),
            last_used_at: chrono::Utc::now(),
            is_active: true,
        };

        // Update registry
        self.add_worktree_to_registry(&worktree_info, agent_id.as_deref())
            .await?;

        Ok(worktree_info)
    }

    /// List all git worktrees
    pub async fn list_worktrees(&self) -> Result<Vec<GitWorktreeInfo>> {
        let registry = self.load_registry().await?;
        Ok(registry.worktrees.into_values().collect())
    }

    /// Get worktree information by path
    pub async fn get_worktree(&self, worktree_path: &Path) -> Result<GitWorktreeInfo> {
        let registry = self.load_registry().await?;
        let canonical = tokio::fs::canonicalize(worktree_path)
            .await
            .unwrap_or_else(|_| worktree_path.to_path_buf());
        let path_key = canonical.display().to_string();

        registry
            .worktrees
            .get(&path_key)
            .cloned()
            .ok_or_else(|| Error::NotFound {
                entity_type: "worktree".to_string(),
                id: path_key,
            })
    }

    /// Assign an agent to a worktree
    pub async fn assign_agent_to_worktree(
        &self,
        agent_id: &str,
        worktree_path: &Path,
    ) -> Result<()> {
        let mut registry = self.load_registry().await?;
        let path_key = worktree_path.display().to_string();

        // Check if worktree exists
        if let Some(worktree_info) = registry.worktrees.get_mut(&path_key) {
            worktree_info.agent_id = Some(agent_id.to_string());
            worktree_info.last_used_at = chrono::Utc::now();
            worktree_info.is_active = true;

            registry.updated_at = chrono::Utc::now();
            self.save_registry(&registry).await?;

            Ok(())
        } else {
            Err(Error::NotFound {
                entity_type: "worktree".to_string(),
                id: path_key,
            })
        }
    }

    /// Remove git worktree and clean up
    pub async fn remove_worktree(&self, worktree_path: &Path) -> Result<()> {
        let path_str = worktree_path.display().to_string();

        // Remove from git
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("worktree")
            .arg("remove")
            .arg(worktree_path)
            .arg("--force");

        use std::time::Duration;
        let output = tokio::time::timeout(Duration::from_secs(15), cmd.output())
            .await
            .map_err(|_| Error::Execution {
                message: "Timed out executing git worktree remove".into(),
            })?
            .map_err(|e| Error::Execution {
                message: format!("Failed to execute git worktree remove: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "Git worktree remove failed (code {:?}): {}",
                output.status.code(),
                stderr
            );
        }

        // Remove from registry
        self.remove_worktree_from_registry(&path_str).await?;

        Ok(())
    }

    /// Cleanup inactive worktrees
    pub async fn cleanup_inactive_worktrees(
        &self,
        inactive_threshold: chrono::Duration,
    ) -> Result<Vec<String>> {
        let registry = self.load_registry().await?;
        let now = chrono::Utc::now();
        let mut cleaned_paths = Vec::new();

        for (path_str, worktree_info) in &registry.worktrees {
            let inactive_duration = now.signed_duration_since(worktree_info.last_used_at);

            if !worktree_info.is_active || inactive_duration > inactive_threshold {
                match self.remove_worktree(&worktree_info.path).await {
                    Ok(()) => {
                        cleaned_paths.push(path_str.clone());
                        info!("Cleaned up inactive worktree: {}", path_str);
                    }
                    Err(e) => {
                        warn!("Failed to cleanup worktree {}: {}", path_str, e);
                    }
                }
            }
        }

        Ok(cleaned_paths)
    }

    /// Get worktree assignment for an agent
    pub async fn get_agent_worktree(&self, agent_id: &str) -> Result<Option<GitWorktreeInfo>> {
        let registry = self.load_registry().await?;

        for worktree_info in registry.worktrees.values() {
            if worktree_info.agent_id.as_ref() == Some(&agent_id.to_string()) {
                return Ok(Some(worktree_info.clone()));
            }
        }

        Ok(None)
    }

    /// Add worktree to registry
    async fn add_worktree_to_registry(
        &self,
        worktree_info: &GitWorktreeInfo,
        _agent_id: Option<&str>,
    ) -> Result<()> {
        let mut registry = self.load_registry().await?;
        let canonical = tokio::fs::canonicalize(&worktree_info.path)
            .await
            .unwrap_or_else(|_| worktree_info.path.clone());
        let path_key = canonical.display().to_string();

        registry.worktrees.insert(path_key, worktree_info.clone());
        registry.updated_at = chrono::Utc::now();

        self.save_registry(&registry).await
    }

    /// Remove worktree from registry
    async fn remove_worktree_from_registry(&self, worktree_path: &str) -> Result<()> {
        let mut registry = self.load_registry().await?;

        if registry.worktrees.remove(worktree_path).is_some() {
            registry.updated_at = chrono::Utc::now();
            self.save_registry(&registry).await?;
        }

        Ok(())
    }

    /// Load the workspace registry
    async fn load_registry(&self) -> Result<WorkspaceRegistry> {
        if !self.registry_path.exists() {
            return Ok(WorkspaceRegistry::default());
        }

        let content = fs::read_to_string(&self.registry_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read registry: {}", e),
            })?;

        serde_json::from_str(&content).map_err(|e| Error::Parsing {
            message: format!("Failed to parse registry: {}", e),
        })
    }

    /// Save the workspace registry
    async fn save_registry(&self, registry: &WorkspaceRegistry) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.registry_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| Error::Io {
                message: format!("Failed to create registry directory: {}", e),
            })?;
        }

        let content = serde_json::to_string_pretty(registry)
            .map_err(|e| Error::Serialization(format!("Failed to serialize registry: {}", e)))?;

        fs::write(&self.registry_path, content)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to write registry: {}", e),
            })
    }

    /// Add workspace to registry
    async fn add_to_registry(&self, config: &WorkspaceConfiguration) -> Result<()> {
        let mut registry = self.load_registry().await?;
        registry
            .workspaces
            .insert(config.name.clone(), config.clone());
        registry.updated_at = chrono::Utc::now();
        self.save_registry(&registry).await
    }

    /// Update workspace in registry
    async fn update_in_registry(&self, config: &WorkspaceConfiguration) -> Result<()> {
        let mut registry = self.load_registry().await?;
        if registry.workspaces.contains_key(&config.name) {
            registry
                .workspaces
                .insert(config.name.clone(), config.clone());
            registry.updated_at = chrono::Utc::now();
            self.save_registry(&registry).await?;
        }
        Ok(())
    }

    /// Remove workspace from registry
    async fn remove_from_registry(&self, name: &str) -> Result<()> {
        let mut registry = self.load_registry().await?;
        if registry.workspaces.remove(name).is_some() {
            registry.updated_at = chrono::Utc::now();
            self.save_registry(&registry).await?;
        }
        Ok(())
    }

    /// Validate workspace name
    fn validate_workspace_name(&self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Workspace name cannot be empty".to_string(),
            });
        }

        if name.len() > 100 {
            return Err(Error::Validation {
                message: "Workspace name cannot exceed 100 characters".to_string(),
            });
        }

        // Check for invalid characters
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(Error::Validation {
                message: "Workspace name can only contain alphanumeric characters, hyphens, and underscores".to_string(),
            });
        }

        // Reserved names
        if name == "registry" || name == ".." || name == "." {
            return Err(Error::Validation {
                message: format!("'{}' is a reserved workspace name", name),
            });
        }

        Ok(())
    }

    /// Create project structure in workspace
    async fn create_project_structure(
        &self,
        project_path: &Path,
        structure: &[ProjectItem],
    ) -> Result<()> {
        for item in structure {
            let item_path = project_path.join(&item.path);

            match item.item_type {
                ProjectItemType::Directory => {
                    fs::create_dir_all(&item_path)
                        .await
                        .map_err(|e| Error::Io {
                            message: format!(
                                "Failed to create directory {}: {}",
                                item_path.display(),
                                e
                            ),
                        })?;
                }
                ProjectItemType::File => {
                    // Create parent directories if they don't exist
                    if let Some(parent) = item_path.parent() {
                        fs::create_dir_all(parent).await.map_err(|e| Error::Io {
                            message: format!("Failed to create parent directory: {}", e),
                        })?;
                    }

                    let content = item.content.as_deref().unwrap_or("");
                    fs::write(&item_path, content)
                        .await
                        .map_err(|e| Error::Io {
                            message: format!("Failed to write file {}: {}", item_path.display(), e),
                        })?;
                }
            }
        }

        Ok(())
    }

    /// Clone git repository into project directory
    async fn clone_git_repository(
        &self,
        project_path: &Path,
        git_config: &GitConfig,
    ) -> Result<()> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("clone").arg(&git_config.url).arg(project_path);

        if let Some(branch) = &git_config.branch {
            cmd.arg("--branch").arg(branch);
        }

        use std::time::Duration;
        let output = tokio::time::timeout(Duration::from_secs(60), cmd.output()) // Clone can take longer
            .await
            .map_err(|_| Error::Execution {
                message: "Timed out executing git clone".into(),
            })?
            .map_err(|e| Error::Execution {
                message: format!("Failed to execute git clone: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Execution {
                message: format!("Git clone failed: {}", stderr),
            });
        }

        // Checkout specific ref if provided
        if let Some(ref_name) = &git_config.ref_name {
            let mut checkout_cmd = tokio::process::Command::new("git");
            checkout_cmd
                .arg("checkout")
                .arg(ref_name)
                .current_dir(project_path);

            let checkout_output = checkout_cmd.output().await.map_err(|e| Error::Execution {
                message: format!("Failed to execute git checkout: {}", e),
            })?;

            if !checkout_output.status.success() {
                let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                return Err(Error::Execution {
                    message: format!("Git checkout failed: {}", stderr),
                });
            }
        }

        Ok(())
    }

    /// Calculate directory statistics recursively
    async fn calculate_directory_stats(
        &self,
        path: &Path,
        stats: &mut WorkspaceStats,
    ) -> Result<()> {
        let mut entries = fs::read_dir(path).await.map_err(|e| Error::Io {
            message: format!("Failed to read directory {}: {}", path.display(), e),
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io {
            message: format!("Failed to read directory entry: {}", e),
        })? {
            let entry_path = entry.path();
            let metadata = entry.metadata().await.map_err(|e| Error::Io {
                message: format!("Failed to get metadata for {}: {}", entry_path.display(), e),
            })?;

            if metadata.is_dir() {
                stats.directory_count += 1;
                Box::pin(self.calculate_directory_stats(&entry_path, stats)).await?;
            } else {
                stats.file_count += 1;
                stats.total_size_bytes += metadata.len();
            }
        }

        Ok(())
    }
}

/// Statistics about a workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStats {
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
    pub template_name: String,
    pub template_version: String,
    pub file_count: u64,
    pub directory_count: u64,
    pub total_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::models::{
        AgentTemplateMetadata, TemplateVariable, TemplateVariableType, ToolPermissions,
    };
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_test_template() -> FilesystemTemplate {
        let metadata = AgentTemplateMetadata {
            name: "test-template".to_string(),
            description: "Test template".to_string(),
            version: "1.0.0".to_string(),
            author: Some("Test Author".to_string()),
            variables: vec![TemplateVariable::new(
                "project_name".to_string(),
                "Name of the project".to_string(),
                TemplateVariableType::String,
                true,
            )
            .unwrap()],
            capabilities: vec!["test".to_string()],
            tool_permissions: ToolPermissions::default(),
            tags: Vec::new(),
            min_claude_version: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        FilesystemTemplate {
            metadata,
            path: PathBuf::from("/tmp/templates/test"),
            config_template: "Test config for {{project_name}}".to_string(),
            prompt_templates: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_workspace_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        assert_eq!(manager.workspaces_directory, temp_dir.path());
    }

    #[tokio::test]
    async fn test_create_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        let workspace = manager
            .create_workspace("test-workspace", &template, &config)
            .await
            .unwrap();

        assert_eq!(workspace.name, "test-workspace");
        assert_eq!(workspace.template_name, "test-template");
        assert!(workspace.is_active);

        // Check directory structure
        assert!(workspace.workspace_path.exists());
        assert!(workspace.project_path.exists());
        assert!(workspace.agent_config_path.exists());

        // Check workspace config file
        let config_path = workspace.workspace_path.join("workspace.json");
        assert!(config_path.exists());
    }

    #[tokio::test]
    async fn test_create_workspace_with_project_structure() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        config.project_structure = vec![
            ProjectItem {
                path: "src".to_string(),
                item_type: ProjectItemType::Directory,
                content: None,
            },
            ProjectItem {
                path: "src/main.rs".to_string(),
                item_type: ProjectItemType::File,
                content: Some("fn main() {\n    println!(\"Hello, world!\");\n}".to_string()),
            },
            ProjectItem {
                path: "README.md".to_string(),
                item_type: ProjectItemType::File,
                content: Some("# TestProject\n\nA test project.".to_string()),
            },
        ];

        let workspace = manager
            .create_workspace("structured-workspace", &template, &config)
            .await
            .unwrap();

        // Check created structure
        let src_dir = workspace.project_path.join("src");
        assert!(src_dir.exists());
        assert!(src_dir.is_dir());

        let main_file = workspace.project_path.join("src/main.rs");
        assert!(main_file.exists());
        assert!(main_file.is_file());

        let readme_file = workspace.project_path.join("README.md");
        assert!(readme_file.exists());
        assert!(readme_file.is_file());

        let main_content = fs::read_to_string(&main_file).await.unwrap();
        assert!(main_content.contains("Hello, world!"));
    }

    #[tokio::test]
    async fn test_load_workspace_config() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        // Create workspace
        let created_workspace = manager
            .create_workspace("load-test", &template, &config)
            .await
            .unwrap();

        let initial_used_at = created_workspace.last_used_at;

        // Small delay to ensure different timestamp
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Load workspace
        let loaded_workspace = manager.load_workspace_config("load-test").await.unwrap();

        assert_eq!(loaded_workspace.name, created_workspace.name);
        assert_eq!(loaded_workspace.id, created_workspace.id);
        assert!(loaded_workspace.last_used_at > initial_used_at);
    }

    #[tokio::test]
    async fn test_list_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        // Create multiple workspaces
        manager
            .create_workspace("workspace-a", &template, &config)
            .await
            .unwrap();

        manager
            .create_workspace("workspace-b", &template, &config)
            .await
            .unwrap();

        let workspaces = manager.list_workspaces().await.unwrap();

        assert_eq!(workspaces.len(), 2);
        assert!(workspaces.contains(&"workspace-a".to_string()));
        assert!(workspaces.contains(&"workspace-b".to_string()));
    }

    #[tokio::test]
    async fn test_workspace_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        assert!(!manager.workspace_exists("nonexistent").await.unwrap());

        manager
            .create_workspace("existing-workspace", &template, &config)
            .await
            .unwrap();

        assert!(manager
            .workspace_exists("existing-workspace")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        let workspace = manager
            .create_workspace("delete-test", &template, &config)
            .await
            .unwrap();

        assert!(workspace.workspace_path.exists());
        assert!(manager.workspace_exists("delete-test").await.unwrap());

        manager.delete_workspace("delete-test").await.unwrap();

        assert!(!workspace.workspace_path.exists());
        assert!(!manager.workspace_exists("delete-test").await.unwrap());
    }

    #[tokio::test]
    async fn test_workspace_name_validation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();
        let config = WorkspaceConfig::default();

        // Empty name should fail
        let result = manager.create_workspace("", &template, &config).await;
        assert!(result.is_err());

        // Invalid characters should fail
        let result = manager
            .create_workspace("workspace with spaces", &template, &config)
            .await;
        assert!(result.is_err());

        // Reserved name should fail
        let result = manager
            .create_workspace("registry", &template, &config)
            .await;
        assert!(result.is_err());

        // Valid name should succeed
        let mut valid_config = WorkspaceConfig::default();
        valid_config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        let result = manager
            .create_workspace("valid-workspace", &template, &valid_config)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_reuse_existing_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig {
            reuse_existing: true,
            ..Default::default()
        };
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        // Create initial workspace
        let workspace1 = manager
            .create_workspace("reuse-test", &template, &config)
            .await
            .unwrap();

        // Create "same" workspace again (should reuse)
        let workspace2 = manager
            .create_workspace("reuse-test", &template, &config)
            .await
            .unwrap();

        assert_eq!(workspace1.id, workspace2.id);
        assert_eq!(workspace1.name, workspace2.name);

        // Test with reuse disabled
        config.reuse_existing = false;
        let result = manager
            .create_workspace("reuse-test", &template, &config)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::AlreadyExists { .. }));
    }

    #[tokio::test]
    async fn test_get_workspace_stats() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());
        let template = create_test_template();

        let mut config = WorkspaceConfig::default();
        config
            .variables
            .insert("project_name".to_string(), "TestProject".to_string());

        config.project_structure = vec![
            ProjectItem {
                path: "src".to_string(),
                item_type: ProjectItemType::Directory,
                content: None,
            },
            ProjectItem {
                path: "src/main.rs".to_string(),
                item_type: ProjectItemType::File,
                content: Some("fn main() {}".to_string()),
            },
            ProjectItem {
                path: "README.md".to_string(),
                item_type: ProjectItemType::File,
                content: Some("# Project".to_string()),
            },
        ];

        manager
            .create_workspace("stats-test", &template, &config)
            .await
            .unwrap();

        let stats = manager.get_workspace_stats("stats-test").await.unwrap();

        assert_eq!(stats.name, "stats-test");
        assert_eq!(stats.template_name, "test-template");
        assert_eq!(stats.template_version, "1.0.0");
        assert!(stats.is_active);
        assert!(stats.file_count > 0); // At least workspace.json + created files
        assert!(stats.directory_count > 0); // At least .claude and project dirs
        assert!(stats.total_size_bytes > 0);
    }

    #[tokio::test]
    async fn test_create_git_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        // Create a mock git repository directory
        let git_repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir_all(&git_repo_dir).await.unwrap();

        // Initialize a git repo (we'll skip actual git commands for unit tests)
        let config = GitWorktreeConfig {
            main_repo_path: git_repo_dir.clone(),
            branch_name: "feature-branch".to_string(),
            create_branch: true,
            base_branch: Some("main".to_string()),
        };

        // Note: This test would fail without an actual git repo, so we'll test the validation
        let result = manager
            .create_worktree("test-worktree", &config, Some("agent-1".to_string()))
            .await;

        // The test should fail with git command execution, but the validation should pass
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        // The error should be related to git execution or worktree creation
        assert!(
            error_message.contains("git")
                || error_message.contains("worktree")
                || error_message.contains("Failed to execute")
        );
    }

    #[tokio::test]
    async fn test_list_empty_worktrees() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let worktrees = manager.list_worktrees().await.unwrap();
        assert!(worktrees.is_empty());
    }

    #[tokio::test]
    async fn test_get_nonexistent_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let result = manager
            .get_worktree(&temp_dir.path().join("nonexistent"))
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_assign_agent_to_nonexistent_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let result = manager
            .assign_agent_to_worktree("agent-1", &temp_dir.path().join("nonexistent"))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_remove_nonexistent_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        // Should not fail - removing nonexistent worktree should be idempotent
        let result = manager
            .remove_worktree(&temp_dir.path().join("nonexistent"))
            .await;

        // The registry operation should succeed even if git command fails
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_inactive_worktrees() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        // Create some mock worktree entries in registry
        let mut registry = WorkspaceRegistry::default();

        let old_worktree = GitWorktreeInfo {
            path: temp_dir.path().join("old-worktree"),
            branch: "old-branch".to_string(),
            agent_id: Some("agent-1".to_string()),
            created_at: chrono::Utc::now() - chrono::Duration::days(2),
            last_used_at: chrono::Utc::now() - chrono::Duration::days(2),
            is_active: false,
        };

        let new_worktree = GitWorktreeInfo {
            path: temp_dir.path().join("new-worktree"),
            branch: "new-branch".to_string(),
            agent_id: Some("agent-2".to_string()),
            created_at: chrono::Utc::now(),
            last_used_at: chrono::Utc::now(),
            is_active: true,
        };

        registry
            .worktrees
            .insert(old_worktree.path.display().to_string(), old_worktree);
        registry
            .worktrees
            .insert(new_worktree.path.display().to_string(), new_worktree);

        // Manually save registry for test
        manager.save_registry(&registry).await.unwrap();

        // Test cleanup with 1 hour threshold (should clean old worktree)
        let cleaned_paths = manager
            .cleanup_inactive_worktrees(chrono::Duration::hours(1))
            .await
            .unwrap();

        assert_eq!(cleaned_paths.len(), 1);
        assert!(cleaned_paths[0].contains("old-worktree"));
    }

    #[tokio::test]
    async fn test_get_agent_worktree_none() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let result = manager
            .get_agent_worktree("nonexistent-agent")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_worktree_registry_operations() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        let worktree_info = GitWorktreeInfo {
            path: temp_dir.path().join("test-worktree"),
            branch: "test-branch".to_string(),
            agent_id: Some("test-agent".to_string()),
            created_at: chrono::Utc::now(),
            last_used_at: chrono::Utc::now(),
            is_active: true,
        };

        // Test adding to registry
        manager
            .add_worktree_to_registry(&worktree_info, worktree_info.agent_id.as_deref())
            .await
            .unwrap();

        // Test loading registry
        let registry = manager.load_registry().await.unwrap();
        assert_eq!(registry.worktrees.len(), 1);
        assert!(registry
            .worktrees
            .contains_key(&worktree_info.path.display().to_string()));

        // Test removing from registry
        manager
            .remove_worktree_from_registry(&worktree_info.path.display().to_string())
            .await
            .unwrap();

        let registry_after_remove = manager.load_registry().await.unwrap();
        assert!(registry_after_remove.worktrees.is_empty());
    }

    #[tokio::test]
    async fn test_git_worktree_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path());

        // Test with invalid worktree name
        let config = GitWorktreeConfig {
            main_repo_path: temp_dir.path().to_path_buf(),
            branch_name: "test-branch".to_string(),
            create_branch: true,
            base_branch: None,
        };

        let result = manager
            .create_worktree("invalid name with spaces", &config, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("alphanumeric"));
    }
}
