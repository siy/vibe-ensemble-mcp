//! Data models for the Claude Code agent orchestration system
//!
//! This module defines the core data structures used throughout the orchestration
//! system, including template metadata, workspace configurations, and execution results.

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Template metadata loaded from template.json files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTemplateMetadata {
    /// Template name and identifier
    pub name: String,
    /// Human-readable description of the template
    pub description: String,
    /// Version of the template
    pub version: String,
    /// Author or maintainer of the template
    pub author: Option<String>,
    /// Template variables that can be substituted
    pub variables: Vec<TemplateVariable>,
    /// Capabilities that agents created from this template will have
    pub capabilities: Vec<String>,
    /// Tool permissions for Claude Code execution
    pub tool_permissions: ToolPermissions,
    /// Optional tags for categorization
    pub tags: Vec<String>,
    /// Minimum Claude Code version required
    pub min_claude_version: Option<String>,
    /// When this template was created
    pub created_at: DateTime<Utc>,
    /// When this template was last updated
    pub updated_at: DateTime<Utc>,
}

/// A template variable that can be substituted during agent configuration generation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateVariable {
    /// Variable name (used in Handlebars templates)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Data type of the variable
    pub variable_type: TemplateVariableType,
    /// Default value if none provided
    pub default_value: Option<String>,
    /// Whether this variable is required
    pub required: bool,
    /// Validation constraints
    pub constraints: Option<VariableConstraints>,
}

/// Supported template variable types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateVariableType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

/// Validation constraints for template variables
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableConstraints {
    /// Minimum length (for strings) or value (for numbers)
    pub min: Option<f64>,
    /// Maximum length (for strings) or value (for numbers)
    pub max: Option<f64>,
    /// Pattern for string validation (regex)
    pub pattern: Option<String>,
    /// Allowed values (enum)
    pub allowed_values: Option<Vec<String>>,
}

/// Tool permissions configuration for Claude Code execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolPermissions {
    /// Allowed tools (empty means all allowed)
    pub allowed_tools: Vec<String>,
    /// Explicitly denied tools
    pub denied_tools: Vec<String>,
    /// File system access permissions
    pub file_access: FileAccessConfig,
    /// Network access permissions
    pub network_access: NetworkAccessConfig,
    /// Process execution permissions
    pub process_access: ProcessAccessConfig,
}

/// File system access configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileAccessConfig {
    /// Allow read access to files
    pub read: bool,
    /// Allow write access to files
    pub write: bool,
    /// Allowed paths (empty means workspace-only)
    pub allowed_paths: Vec<String>,
    /// Denied paths
    pub denied_paths: Vec<String>,
}

/// Network access configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkAccessConfig {
    /// Allow network access
    pub enabled: bool,
    /// Allowed domains (empty means all allowed if enabled)
    pub allowed_domains: Vec<String>,
    /// Denied domains
    pub denied_domains: Vec<String>,
}

/// Process execution configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessAccessConfig {
    /// Allow process execution
    pub enabled: bool,
    /// Allowed commands (empty means all allowed if enabled)
    pub allowed_commands: Vec<String>,
    /// Denied commands
    pub denied_commands: Vec<String>,
}

/// Filesystem-based agent template loaded from disk
#[derive(Debug, Clone)]
pub struct FilesystemTemplate {
    /// Template metadata from template.json
    pub metadata: AgentTemplateMetadata,
    /// Path to the template directory
    pub path: PathBuf,
    /// Agent configuration template content (from agent-config.md)
    pub config_template: String,
    /// Prompt templates (from prompts/ directory)
    pub prompt_templates: HashMap<String, String>,
}

/// Workspace configuration for an instantiated agent template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfiguration {
    /// Unique workspace identifier
    pub id: Uuid,
    /// Workspace name
    pub name: String,
    /// Template that was used to create this workspace
    pub template_name: String,
    /// Template version used
    pub template_version: String,
    /// Path to the workspace directory
    pub workspace_path: PathBuf,
    /// Project directory within the workspace
    pub project_path: PathBuf,
    /// Agent configuration path (.claude/agents/)
    pub agent_config_path: PathBuf,
    /// Variable values used during instantiation
    pub variable_values: HashMap<String, String>,
    /// Capabilities of the agent in this workspace
    pub capabilities: Vec<String>,
    /// Tool permissions
    pub tool_permissions: ToolPermissions,
    /// When this workspace was created
    pub created_at: DateTime<Utc>,
    /// When this workspace was last used
    pub last_used_at: DateTime<Utc>,
    /// Whether this workspace is currently active
    pub is_active: bool,
}

/// Context for workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionContext {
    /// Workflow identifier
    pub workflow_id: Uuid,
    /// Current step being executed
    pub current_step: String,
    /// Variables available to the workflow
    pub variables: HashMap<String, String>,
    /// Results from previous steps
    pub step_results: HashMap<String, serde_json::Value>,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
    /// Start time of workflow execution
    pub started_at: DateTime<Utc>,
    /// Current status
    pub status: WorkflowStatus,
}

/// Status of workflow execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Result of a workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    /// Workflow that was executed
    pub workflow_id: Uuid,
    /// Final status
    pub status: WorkflowStatus,
    /// Results from each step
    pub step_results: HashMap<String, serde_json::Value>,
    /// Any error that occurred
    pub error: Option<String>,
    /// Execution statistics
    pub stats: WorkflowStats,
    /// When execution completed
    pub completed_at: DateTime<Utc>,
}

/// Statistics about workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStats {
    /// Total execution time in milliseconds
    pub total_duration_ms: u64,
    /// Number of steps executed
    pub steps_executed: u32,
    /// Number of retries across all steps
    pub total_retries: u32,
    /// Maximum memory usage in MB
    pub peak_memory_mb: Option<f64>,
    /// Total cost in dollars (if available)
    pub total_cost: Option<f64>,
}

impl Default for ToolPermissions {
    fn default() -> Self {
        Self {
            allowed_tools: Vec::new(), // Empty means all allowed
            denied_tools: Vec::new(),
            file_access: FileAccessConfig::default(),
            network_access: NetworkAccessConfig::default(),
            process_access: ProcessAccessConfig::default(),
        }
    }
}

impl Default for FileAccessConfig {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            allowed_paths: Vec::new(), // Empty means workspace-only
            denied_paths: Vec::new(),
        }
    }
}

impl Default for NetworkAccessConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_domains: Vec::new(), // Empty means all allowed if enabled
            denied_domains: Vec::new(),
        }
    }
}

impl Default for ProcessAccessConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_commands: Vec::new(), // Empty means all allowed if enabled
            denied_commands: Vec::new(),
        }
    }
}

impl TemplateVariable {
    /// Create a new template variable with validation
    pub fn new(
        name: String,
        description: String,
        variable_type: TemplateVariableType,
        required: bool,
    ) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Template variable name cannot be empty".to_string(),
            });
        }

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(Error::Validation {
                message: "Template variable name can only contain alphanumeric characters, underscores, and hyphens".to_string(),
            });
        }

        Ok(Self {
            name,
            description,
            variable_type,
            default_value: None,
            required,
            constraints: None,
        })
    }

    /// Set the default value for this variable
    pub fn with_default_value(mut self, default_value: String) -> Self {
        self.default_value = Some(default_value);
        self
    }

    /// Set validation constraints for this variable
    pub fn with_constraints(mut self, constraints: VariableConstraints) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Validate a value against this variable's constraints
    pub fn validate_value(&self, value: &str) -> Result<()> {
        if let Some(constraints) = &self.constraints {
            // Check pattern
            if let Some(pattern) = &constraints.pattern {
                let regex = regex::Regex::new(pattern).map_err(|e| Error::Validation {
                    message: format!("Invalid regex pattern '{}': {}", pattern, e),
                })?;
                if !regex.is_match(value) {
                    return Err(Error::Validation {
                        message: format!(
                            "Value '{}' does not match pattern '{}' for variable '{}'",
                            value, pattern, self.name
                        ),
                    });
                }
            }

            // Check allowed values
            if let Some(allowed_values) = &constraints.allowed_values {
                if !allowed_values.contains(&value.to_string()) {
                    return Err(Error::Validation {
                        message: format!(
                            "Value '{}' is not in allowed values {:?} for variable '{}'",
                            value, allowed_values, self.name
                        ),
                    });
                }
            }

            // Type-specific validation
            match self.variable_type {
                TemplateVariableType::String => {
                    if let Some(min) = constraints.min {
                        if (value.len() as f64) < min {
                            return Err(Error::Validation {
                                message: format!(
                                    "String value '{}' is shorter than minimum length {} for variable '{}'",
                                    value, min, self.name
                                ),
                            });
                        }
                    }
                    if let Some(max) = constraints.max {
                        if (value.len() as f64) > max {
                            return Err(Error::Validation {
                                message: format!(
                                    "String value '{}' is longer than maximum length {} for variable '{}'",
                                    value, max, self.name
                                ),
                            });
                        }
                    }
                }
                TemplateVariableType::Number => {
                    let num: f64 = value.parse().map_err(|_| Error::Validation {
                        message: format!(
                            "Value '{}' is not a valid number for variable '{}'",
                            value, self.name
                        ),
                    })?;
                    if let Some(min) = constraints.min {
                        if num < min {
                            return Err(Error::Validation {
                                message: format!(
                                    "Number value {} is less than minimum {} for variable '{}'",
                                    num, min, self.name
                                ),
                            });
                        }
                    }
                    if let Some(max) = constraints.max {
                        if num > max {
                            return Err(Error::Validation {
                                message: format!(
                                    "Number value {} is greater than maximum {} for variable '{}'",
                                    num, max, self.name
                                ),
                            });
                        }
                    }
                }
                TemplateVariableType::Boolean => {
                    value.parse::<bool>().map_err(|_| Error::Validation {
                        message: format!(
                            "Value '{}' is not a valid boolean for variable '{}'",
                            value, self.name
                        ),
                    })?;
                }
                TemplateVariableType::Array | TemplateVariableType::Object => {
                    serde_json::from_str::<serde_json::Value>(value).map_err(|e| {
                        Error::Validation {
                            message: format!(
                                "Value '{}' is not valid JSON for variable '{}': {}",
                                value, self.name, e
                            ),
                        }
                    })?;
                }
            }
        }

        Ok(())
    }
}

impl WorkspaceConfiguration {
    /// Create a new workspace configuration
    pub fn new(
        name: String,
        template: &FilesystemTemplate,
        workspace_path: PathBuf,
        variable_values: HashMap<String, String>,
    ) -> Self {
        let now = Utc::now();
        let project_path = workspace_path.join("project");
        let agent_config_path = workspace_path.join(".claude").join("agents");

        Self {
            id: Uuid::new_v4(),
            name,
            template_name: template.metadata.name.clone(),
            template_version: template.metadata.version.clone(),
            workspace_path,
            project_path,
            agent_config_path,
            variable_values,
            capabilities: template.metadata.capabilities.clone(),
            tool_permissions: template.metadata.tool_permissions.clone(),
            created_at: now,
            last_used_at: now,
            is_active: true,
        }
    }

    /// Mark this workspace as used
    pub fn mark_used(&mut self) {
        self.last_used_at = Utc::now();
    }

    /// Activate the workspace
    pub fn activate(&mut self) {
        self.is_active = true;
        self.last_used_at = Utc::now();
    }

    /// Deactivate the workspace
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_variable_creation() {
        let var = TemplateVariable::new(
            "project_name".to_string(),
            "Name of the project".to_string(),
            TemplateVariableType::String,
            true,
        );

        assert!(var.is_ok());
        let var = var.unwrap();
        assert_eq!(var.name, "project_name");
        assert_eq!(var.variable_type, TemplateVariableType::String);
        assert!(var.required);
    }

    #[test]
    fn test_template_variable_validation() {
        // Empty name should fail
        let result = TemplateVariable::new(
            "".to_string(),
            "Description".to_string(),
            TemplateVariableType::String,
            false,
        );
        assert!(result.is_err());

        // Invalid characters should fail
        let result = TemplateVariable::new(
            "invalid name!".to_string(),
            "Description".to_string(),
            TemplateVariableType::String,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_variable_value_validation() {
        let mut var = TemplateVariable::new(
            "port".to_string(),
            "Port number".to_string(),
            TemplateVariableType::Number,
            true,
        )
        .unwrap();

        // Add constraints
        var = var.with_constraints(VariableConstraints {
            min: Some(1024.0),
            max: Some(65535.0),
            pattern: None,
            allowed_values: None,
        });

        // Valid number within range
        assert!(var.validate_value("8080").is_ok());

        // Invalid number (below min)
        assert!(var.validate_value("80").is_err());

        // Invalid number (above max)
        assert!(var.validate_value("70000").is_err());

        // Invalid non-number
        assert!(var.validate_value("not-a-number").is_err());
    }

    #[test]
    fn test_string_validation_with_pattern() {
        let mut var = TemplateVariable::new(
            "email".to_string(),
            "Email address".to_string(),
            TemplateVariableType::String,
            true,
        )
        .unwrap();

        // Add email pattern constraint
        var = var.with_constraints(VariableConstraints {
            min: None,
            max: None,
            pattern: Some(r"^[^@]+@[^@]+\.[^@]+$".to_string()),
            allowed_values: None,
        });

        // Valid email
        assert!(var.validate_value("test@example.com").is_ok());

        // Invalid email
        assert!(var.validate_value("not-an-email").is_err());
    }

    #[test]
    fn test_allowed_values_validation() {
        let mut var = TemplateVariable::new(
            "environment".to_string(),
            "Environment type".to_string(),
            TemplateVariableType::String,
            true,
        )
        .unwrap();

        // Add allowed values constraint
        var = var.with_constraints(VariableConstraints {
            min: None,
            max: None,
            pattern: None,
            allowed_values: Some(vec![
                "dev".to_string(),
                "staging".to_string(),
                "prod".to_string(),
            ]),
        });

        // Valid value
        assert!(var.validate_value("dev").is_ok());
        assert!(var.validate_value("staging").is_ok());
        assert!(var.validate_value("prod").is_ok());

        // Invalid value
        assert!(var.validate_value("test").is_err());
    }

    #[test]
    fn test_workspace_configuration_creation() {
        let now = Utc::now();
        let metadata = AgentTemplateMetadata {
            name: "test-template".to_string(),
            description: "Test template".to_string(),
            version: "1.0.0".to_string(),
            author: Some("Test Author".to_string()),
            variables: Vec::new(),
            capabilities: vec!["test".to_string()],
            tool_permissions: ToolPermissions::default(),
            tags: Vec::new(),
            min_claude_version: None,
            created_at: now,
            updated_at: now,
        };

        let template = FilesystemTemplate {
            metadata,
            path: PathBuf::from("/tmp/templates/test"),
            config_template: "Test config".to_string(),
            prompt_templates: HashMap::new(),
        };

        let workspace_path = PathBuf::from("/tmp/workspaces/test-workspace");
        let variables = HashMap::new();

        let config = WorkspaceConfiguration::new(
            "test-workspace".to_string(),
            &template,
            workspace_path.clone(),
            variables,
        );

        assert_eq!(config.name, "test-workspace");
        assert_eq!(config.template_name, "test-template");
        assert_eq!(config.workspace_path, workspace_path);
        assert_eq!(config.project_path, workspace_path.join("project"));
        assert!(config.is_active);
    }

    #[test]
    fn test_workspace_lifecycle() {
        let now = Utc::now();
        let metadata = AgentTemplateMetadata {
            name: "test-template".to_string(),
            description: "Test template".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            variables: Vec::new(),
            capabilities: Vec::new(),
            tool_permissions: ToolPermissions::default(),
            tags: Vec::new(),
            min_claude_version: None,
            created_at: now,
            updated_at: now,
        };

        let template = FilesystemTemplate {
            metadata,
            path: PathBuf::from("/tmp/templates/test"),
            config_template: "Test config".to_string(),
            prompt_templates: HashMap::new(),
        };

        let mut config = WorkspaceConfiguration::new(
            "test-workspace".to_string(),
            &template,
            PathBuf::from("/tmp/workspaces/test"),
            HashMap::new(),
        );

        assert!(config.is_active);

        config.deactivate();
        assert!(!config.is_active);

        config.activate();
        assert!(config.is_active);

        let initial_used_at = config.last_used_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        config.mark_used();
        assert!(config.last_used_at > initial_used_at);
    }
}
