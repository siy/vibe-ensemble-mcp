//! Project domain model and related types
//!
//! This module provides the core project entity for representing projects
//! in the Vibe Ensemble system. Projects are organizational units that
//! group agents and provide context for their coordination activities.
//!
//! # Examples
//!
//! Creating a new project:
//!
//! ```rust
//! use vibe_ensemble_core::project::*;
//! use std::path::PathBuf;
//!
//! let project = Project::builder()
//!     .name("my-awesome-project")
//!     .description("A revolutionary new application")
//!     .workspace_path("/path/to/project")
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(project.name, "my-awesome-project");
//! assert!(project.workspace_path.is_some());
//! ```

use crate::{Error, Result, ValidationErrors};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Represents a project in the Vibe Ensemble system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub workspace_path: Option<PathBuf>,
}

impl Project {
    /// Create a new project instance with validation
    pub fn new(
        name: String,
        description: Option<String>,
        workspace_path: Option<PathBuf>,
    ) -> Result<Self> {
        let mut validation_errors = ValidationErrors::new();

        // Validate all fields
        validation_errors.add_result(Self::validate_name(&name));
        if let Some(ref desc) = description {
            validation_errors.add_result(Self::validate_description(desc));
        }
        if let Some(ref path) = workspace_path {
            validation_errors.add_result(Self::validate_workspace_path(path));
        }

        // If there are validation errors, return them all at once
        if !validation_errors.is_empty() {
            return Err(validation_errors.into_error().unwrap());
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            created_at: now,
            updated_at: now,
            workspace_path,
        })
    }

    /// Create a builder for constructing a Project
    pub fn builder() -> ProjectBuilder {
        ProjectBuilder::new()
    }

    /// Validate project name
    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::validation("Project name cannot be empty"));
        }
        if name.len() > 200 {
            return Err(Error::constraint_violation(
                "name_length",
                "Project name cannot exceed 200 characters",
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '_' || c == '.')
        {
            return Err(Error::validation(
                "Project name can only contain alphanumeric characters, whitespace, hyphens, underscores, and dots",
            ));
        }
        Ok(())
    }

    /// Validate project description
    fn validate_description(description: &str) -> Result<()> {
        if description.len() > 1000 {
            return Err(Error::constraint_violation(
                "description_length",
                "Project description cannot exceed 1000 characters",
            ));
        }
        Ok(())
    }

    /// Validate workspace path
    fn validate_workspace_path(path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        if path_str.len() > 1000 {
            return Err(Error::constraint_violation(
                "workspace_path_length",
                "Workspace path cannot exceed 1000 characters",
            ));
        }
        Ok(())
    }

    /// Update the project's updated_at timestamp
    pub fn update_timestamp(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Update the project name with validation
    pub fn set_name(&mut self, name: String) -> Result<()> {
        Self::validate_name(&name)?;
        self.name = name;
        self.update_timestamp();
        Ok(())
    }

    /// Update the project description
    pub fn set_description(&mut self, description: Option<String>) -> Result<()> {
        if let Some(ref desc) = description {
            Self::validate_description(desc)?;
        }
        self.description = description;
        self.update_timestamp();
        Ok(())
    }

    /// Update the workspace path with validation
    pub fn set_workspace_path(&mut self, workspace_path: Option<PathBuf>) -> Result<()> {
        if let Some(ref path) = workspace_path {
            Self::validate_workspace_path(path)?;
        }
        self.workspace_path = workspace_path;
        self.update_timestamp();
        Ok(())
    }

    /// Check if the project has a workspace path configured
    pub fn has_workspace(&self) -> bool {
        self.workspace_path.is_some()
    }

    /// Get the workspace path as a string
    pub fn workspace_path_string(&self) -> Option<String> {
        self.workspace_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
    }
}

/// Builder for constructing Project instances with validation
#[derive(Debug, Clone, Default)]
pub struct ProjectBuilder {
    name: Option<String>,
    description: Option<String>,
    workspace_path: Option<PathBuf>,
}

impl ProjectBuilder {
    /// Create a new project builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the project name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the project description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the workspace path from a string
    pub fn workspace_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.workspace_path = Some(path.into());
        self
    }

    /// Build the Project instance
    pub fn build(self) -> Result<Project> {
        let name = self
            .name
            .ok_or_else(|| Error::validation("Project name is required"))?;

        Project::new(name, self.description, self.workspace_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation_with_builder() {
        let project = Project::builder()
            .name("test-project")
            .description("A test project for validation")
            .workspace_path("/path/to/project")
            .build()
            .unwrap();

        assert_eq!(project.name, "test-project");
        assert_eq!(
            project.description,
            Some("A test project for validation".to_string())
        );
        assert!(project.has_workspace());
        assert_eq!(
            project.workspace_path_string(),
            Some("/path/to/project".to_string())
        );
    }

    #[test]
    fn test_project_creation_minimal() {
        let project = Project::builder().name("minimal-project").build().unwrap();

        assert_eq!(project.name, "minimal-project");
        assert_eq!(project.description, None);
        assert!(!project.has_workspace());
    }

    #[test]
    fn test_project_name_validation() {
        // Empty name should fail
        let result = Project::builder().name("").build();
        assert!(result.is_err());

        // Name with invalid characters should fail
        let result = Project::builder().name("test@project").build();
        assert!(result.is_err());

        // Too long name should fail
        let long_name = "a".repeat(201);
        let result = Project::builder().name(long_name).build();
        assert!(result.is_err());

        // Valid names should succeed
        let valid_names = vec![
            "simple-project",
            "My Awesome Project",
            "project_with_underscores",
            "project.with.dots",
            "project123",
            "Project With Spaces",
        ];

        for name in valid_names {
            let result = Project::builder().name(name).build();
            assert!(result.is_ok(), "Name '{}' should be valid", name);
        }
    }

    #[test]
    fn test_project_description_validation() {
        // Too long description should fail
        let long_desc = "a".repeat(1001);
        let result = Project::builder()
            .name("test-project")
            .description(long_desc)
            .build();
        assert!(result.is_err());

        // Valid description should succeed
        let result = Project::builder()
            .name("test-project")
            .description("A reasonable length description")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_project_workspace_path_validation() {
        // Too long path should fail
        let long_path = PathBuf::from("/".to_string() + &"a".repeat(1000));
        let result = Project::builder()
            .name("test-project")
            .workspace_path(long_path)
            .build();
        assert!(result.is_err());

        // Valid path should succeed
        let result = Project::builder()
            .name("test-project")
            .workspace_path("/reasonable/path/to/project")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_project_setters() {
        let mut project = Project::builder().name("original-name").build().unwrap();

        // Test name update
        project.set_name("updated-name".to_string()).unwrap();
        assert_eq!(project.name, "updated-name");

        // Test description update
        project
            .set_description(Some("New description".to_string()))
            .unwrap();
        assert_eq!(project.description, Some("New description".to_string()));

        // Test workspace path update
        project
            .set_workspace_path(Some(PathBuf::from("/new/path")))
            .unwrap();
        assert_eq!(project.workspace_path, Some(PathBuf::from("/new/path")));
        assert!(project.has_workspace());

        // Test clearing workspace path
        project.set_workspace_path(None).unwrap();
        assert!(!project.has_workspace());
    }

    #[test]
    fn test_project_validation_errors() {
        // Test missing required field
        let result = ProjectBuilder::new().build();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.is_validation());

        // Test comprehensive validation errors
        let result = Project::new(
            "".to_string(),                                           // Invalid: empty name
            Some("a".repeat(1001)), // Invalid: too long description
            Some(PathBuf::from("/".to_string() + &"a".repeat(1000))), // Invalid: too long path
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.is_validation());

        // The error should contain information about multiple validation issues
        let error_message = format!("{}", error);
        assert!(
            error_message.contains("Multiple validation errors")
                || error_message.contains("Project name cannot be empty")
        );
    }

    #[test]
    fn test_project_timestamp_updates() {
        let mut project = Project::builder().name("test-project").build().unwrap();

        let initial_updated_at = project.updated_at;

        // Wait a small amount to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        project.set_name("updated-name".to_string()).unwrap();
        assert!(project.updated_at > initial_updated_at);

        let second_updated_at = project.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));

        project
            .set_description(Some("New description".to_string()))
            .unwrap();
        assert!(project.updated_at > second_updated_at);
    }

    #[test]
    fn test_project_workspace_helpers() {
        let mut project = Project::builder().name("test-project").build().unwrap();

        // Initially no workspace
        assert!(!project.has_workspace());
        assert_eq!(project.workspace_path_string(), None);

        // Add workspace
        project
            .set_workspace_path(Some(PathBuf::from("/path/to/project")))
            .unwrap();
        assert!(project.has_workspace());
        assert_eq!(
            project.workspace_path_string(),
            Some("/path/to/project".to_string())
        );

        // Clear workspace
        project.set_workspace_path(None).unwrap();
        assert!(!project.has_workspace());
        assert_eq!(project.workspace_path_string(), None);
    }

    #[test]
    fn test_project_serialization() {
        let project = Project::builder()
            .name("serialization-test")
            .description("Testing JSON serialization")
            .workspace_path("/test/path")
            .build()
            .unwrap();

        // Test serialization
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("serialization-test"));
        assert!(json.contains("Testing JSON serialization"));

        // Test deserialization
        let deserialized: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, project);
    }
}
