//! Filesystem-based agent template management
//!
//! This module provides functionality for loading and managing Claude Code agent templates
//! from the filesystem. Templates are organized in directories with metadata, configuration
//! files, and prompt templates.
//!
//! # Template Structure
//!
//! ```text
//! agent-templates/
//! +-- template-name/
//!     +-- template.json          # Template metadata and variables
//!     +-- agent-config.md        # Agent configuration template (Handlebars)
//!     +-- prompts/               # Optional prompt templates directory
//!         +-- system.md
//!         +-- instructions.md
//!         +-- examples.md
//! ```

use crate::orchestration::models::{AgentTemplateMetadata, FilesystemTemplate};
use crate::{Error, Result};
use async_trait::async_trait;
use handlebars::Handlebars;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Trait for template management operations
#[async_trait]
pub trait TemplateManager {
    /// Load a template by name
    async fn load_template(&self, name: &str) -> Result<FilesystemTemplate>;

    /// List all available templates
    async fn list_templates(&self) -> Result<Vec<String>>;

    /// Check if a template exists
    async fn template_exists(&self, name: &str) -> Result<bool>;

    /// Validate a template structure
    async fn validate_template(&self, template: &FilesystemTemplate) -> Result<()>;

    /// Render agent configuration from template with variables
    async fn render_agent_config(
        &self,
        template: &FilesystemTemplate,
        variables: &HashMap<String, String>,
    ) -> Result<String>;
}

/// Filesystem-based template manager
#[derive(Debug, Clone)]
pub struct FilesystemTemplateManager {
    /// Root directory containing agent templates
    pub templates_directory: PathBuf,
    /// Handlebars template engine
    handlebars: Handlebars<'static>,
}

impl FilesystemTemplateManager {
    /// Create a new filesystem template manager
    pub fn new<P: AsRef<Path>>(templates_directory: P) -> Self {
        let mut handlebars = Handlebars::new();

        // Configure Handlebars with helpful settings
        handlebars.set_strict_mode(true);
        handlebars.register_helper("upper", Box::new(handlebars_helpers::upper));
        handlebars.register_helper("lower", Box::new(handlebars_helpers::lower));
        handlebars.register_helper("json", Box::new(handlebars_helpers::json));
        handlebars.register_helper("eq", Box::new(handlebars_helpers::eq));
        handlebars.register_helper("split", Box::new(handlebars_helpers::split));
        handlebars.register_helper("trim", Box::new(handlebars_helpers::trim));
        handlebars.register_helper("title", Box::new(handlebars_helpers::title));

        Self {
            templates_directory: templates_directory.as_ref().to_path_buf(),
            handlebars,
        }
    }

    /// Get the path to a specific template directory
    fn template_path(&self, name: &str) -> PathBuf {
        self.templates_directory.join(name)
    }

    /// Load template metadata from template.json
    async fn load_template_metadata(&self, template_path: &Path) -> Result<AgentTemplateMetadata> {
        let metadata_path = template_path.join("template.json");

        if !metadata_path.exists() {
            return Err(Error::NotFound {
                entity_type: "template.json".to_string(),
                id: template_path.to_string_lossy().to_string(),
            });
        }

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read template.json: {}", e),
            })?;

        let metadata: AgentTemplateMetadata =
            serde_json::from_str(&content).map_err(|e| Error::Parsing {
                message: format!("Failed to parse template.json: {}", e),
            })?;

        Ok(metadata)
    }

    /// Load agent configuration template from agent-config.md
    async fn load_agent_config_template(&self, template_path: &Path) -> Result<String> {
        let config_path = template_path.join("agent-config.md");

        if !config_path.exists() {
            return Err(Error::NotFound {
                entity_type: "agent-config.md".to_string(),
                id: template_path.to_string_lossy().to_string(),
            });
        }

        fs::read_to_string(&config_path)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read agent-config.md: {}", e),
            })
    }

    /// Load prompt templates from prompts/ directory
    async fn load_prompt_templates(&self, template_path: &Path) -> Result<HashMap<String, String>> {
        let prompts_dir = template_path.join("prompts");
        let mut prompt_templates = HashMap::new();

        if !prompts_dir.exists() {
            return Ok(prompt_templates); // Prompts directory is optional
        }

        let mut entries = fs::read_dir(&prompts_dir).await.map_err(|e| Error::Io {
            message: format!("Failed to read prompts directory: {}", e),
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io {
            message: format!("Failed to read directory entry: {}", e),
        })? {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "md" || extension == "txt" {
                        if let Some(file_stem) = path.file_stem() {
                            let name = file_stem.to_string_lossy().to_string();
                            let content =
                                fs::read_to_string(&path).await.map_err(|e| Error::Io {
                                    message: format!(
                                        "Failed to read prompt file {}: {}",
                                        path.display(),
                                        e
                                    ),
                                })?;
                            prompt_templates.insert(name, content);
                        }
                    }
                }
            }
        }

        Ok(prompt_templates)
    }

    /// Validate variable values against template constraints
    fn validate_variables(
        &self,
        template: &FilesystemTemplate,
        variables: &HashMap<String, String>,
    ) -> Result<()> {
        // Check required variables
        for template_var in &template.metadata.variables {
            if template_var.required
                && !variables.contains_key(&template_var.name)
                && template_var.default_value.is_none()
            {
                return Err(Error::Validation {
                    message: format!(
                        "Required variable '{}' not provided",
                        template_var.name
                    ),
                });
            }
        }

        // Validate provided variable values
        for (name, value) in variables {
            if let Some(template_var) = template.metadata.variables.iter().find(|v| v.name == *name)
            {
                template_var.validate_value(value)?;
            }
        }

        Ok(())
    }

    /// Prepare variables for template rendering
    fn prepare_variables(
        &self,
        template: &FilesystemTemplate,
        variables: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let mut prepared_vars = variables.clone();

        // Add default values for missing variables
        for template_var in &template.metadata.variables {
            if !prepared_vars.contains_key(&template_var.name) {
                if let Some(default_value) = &template_var.default_value {
                    prepared_vars.insert(template_var.name.clone(), default_value.clone());
                }
            }
        }

        // Add common built-in variables
        prepared_vars.insert("template_name".to_string(), template.metadata.name.clone());
        prepared_vars.insert(
            "template_version".to_string(),
            template.metadata.version.clone(),
        );
        prepared_vars.insert(
            "template_description".to_string(),
            template.metadata.description.clone(),
        );
        prepared_vars.insert(
            "capabilities".to_string(),
            template.metadata.capabilities.join(", "),
        );

        Ok(prepared_vars)
    }
}

#[async_trait]
impl TemplateManager for FilesystemTemplateManager {
    async fn load_template(&self, name: &str) -> Result<FilesystemTemplate> {
        let template_path = self.template_path(name);

        if !template_path.exists() {
            return Err(Error::NotFound {
                entity_type: "template".to_string(),
                id: name.to_string(),
            });
        }

        if !template_path.is_dir() {
            return Err(Error::Validation {
                message: format!(
                    "Template path '{}' is not a directory",
                    template_path.display()
                ),
            });
        }

        // Load components
        let metadata = self.load_template_metadata(&template_path).await?;
        let config_template = self.load_agent_config_template(&template_path).await?;
        let prompt_templates = self.load_prompt_templates(&template_path).await?;

        let template = FilesystemTemplate {
            metadata,
            path: template_path,
            config_template,
            prompt_templates,
        };

        // Validate the loaded template
        self.validate_template(&template).await?;

        Ok(template)
    }

    async fn list_templates(&self) -> Result<Vec<String>> {
        if !self.templates_directory.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&self.templates_directory)
            .await
            .map_err(|e| Error::Io {
                message: format!("Failed to read templates directory: {}", e),
            })?;

        let mut templates = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io {
            message: format!("Failed to read directory entry: {}", e),
        })? {
            let path = entry.path();
            if path.is_dir() {
                // Check if it has template.json
                let metadata_path = path.join("template.json");
                if metadata_path.exists() {
                    if let Some(name) = path.file_name() {
                        templates.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        templates.sort();
        Ok(templates)
    }

    async fn template_exists(&self, name: &str) -> Result<bool> {
        let template_path = self.template_path(name);
        let metadata_path = template_path.join("template.json");
        let config_path = template_path.join("agent-config.md");

        Ok(template_path.is_dir() && metadata_path.exists() && config_path.exists())
    }

    async fn validate_template(&self, template: &FilesystemTemplate) -> Result<()> {
        // Validate metadata
        if template.metadata.name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Template name cannot be empty".to_string(),
            });
        }

        if template.metadata.version.trim().is_empty() {
            return Err(Error::Validation {
                message: "Template version cannot be empty".to_string(),
            });
        }

        // Validate configuration template syntax
        if let Err(e) = handlebars::Template::compile(&template.config_template) {
            return Err(Error::Validation {
                message: format!(
                    "Invalid Handlebars template syntax in agent-config.md: {}",
                    e
                ),
            });
        }

        // Validate prompt templates syntax
        for (name, content) in &template.prompt_templates {
            if let Err(e) = handlebars::Template::compile(content) {
                return Err(Error::Validation {
                    message: format!(
                        "Invalid Handlebars template syntax in prompt '{}': {}",
                        name, e
                    ),
                });
            }
        }

        // Validate variables
        let mut variable_names = std::collections::HashSet::new();
        for variable in &template.metadata.variables {
            if !variable_names.insert(&variable.name) {
                return Err(Error::Validation {
                    message: format!("Duplicate variable name: {}", variable.name),
                });
            }
        }

        Ok(())
    }

    async fn render_agent_config(
        &self,
        template: &FilesystemTemplate,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        // Validate variables
        self.validate_variables(template, variables)?;

        // Prepare variables with defaults and built-ins
        let prepared_vars = self.prepare_variables(template, variables)?;

        // Render the configuration template
        self.handlebars
            .render_template(&template.config_template, &prepared_vars)
            .map_err(|e| Error::Rendering {
                message: format!("Failed to render agent configuration: {}", e),
            })
    }
}

/// Helper functions for Handlebars templates
mod handlebars_helpers {
    use handlebars::{Context, Handlebars, Helper, JsonRender, Output, RenderContext, RenderError};

    pub fn upper(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("upper helper requires exactly one parameter"))?;

        let value = param.value().render();
        out.write(&value.to_uppercase())?;
        Ok(())
    }

    pub fn lower(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("lower helper requires exactly one parameter"))?;

        let value = param.value().render();
        out.write(&value.to_lowercase())?;
        Ok(())
    }

    pub fn json(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("json helper requires exactly one parameter"))?;

        let json_str = serde_json::to_string_pretty(param.value())
            .map_err(|e| RenderError::new(format!("Failed to serialize to JSON: {}", e)))?;

        out.write(&json_str)?;
        Ok(())
    }

    pub fn eq(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param1 = h
            .param(0)
            .ok_or_else(|| RenderError::new("eq helper requires two parameters"))?;
        let param2 = h
            .param(1)
            .ok_or_else(|| RenderError::new("eq helper requires two parameters"))?;

        let equal = param1.value() == param2.value();
        // This is a block helper that returns true/false for #if conditions
        // The actual rendering is handled by the #if directive
        out.write(&equal.to_string())?;
        Ok(())
    }

    pub fn split(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("split helper requires two parameters"))?;
        let delimiter = h
            .param(1)
            .ok_or_else(|| RenderError::new("split helper requires two parameters"))?;

        let text = param.value().render();
        let delim = delimiter.value().render();

        let parts: Vec<&str> = text.split(&delim).collect();
        let json_array = serde_json::to_string(&parts)
            .map_err(|e| RenderError::new(format!("Failed to serialize split result: {}", e)))?;

        out.write(&json_array)?;
        Ok(())
    }

    pub fn trim(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("trim helper requires exactly one parameter"))?;

        let value = param.value().render();
        out.write(value.trim())?;
        Ok(())
    }

    pub fn title(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("title helper requires exactly one parameter"))?;

        let value = param.value().render();
        let title_case = value
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        out.write(&title_case)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::models::{
        AgentTemplateMetadata, FileAccessConfig, NetworkAccessConfig, ProcessAccessConfig,
        TemplateVariable, TemplateVariableType, ToolPermissions,
    };
    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_template(temp_dir: &TempDir, name: &str) -> Result<PathBuf> {
        let template_dir = temp_dir.path().join(name);
        fs::create_dir_all(&template_dir).await.unwrap();

        // Create template.json
        let metadata = AgentTemplateMetadata {
            name: name.to_string(),
            description: "Test template".to_string(),
            version: "1.0.0".to_string(),
            author: Some("Test Author".to_string()),
            variables: vec![
                TemplateVariable::new(
                    "project_name".to_string(),
                    "Name of the project".to_string(),
                    TemplateVariableType::String,
                    true,
                )
                .unwrap(),
                TemplateVariable::new(
                    "port".to_string(),
                    "Port number".to_string(),
                    TemplateVariableType::Number,
                    false,
                )
                .unwrap()
                .with_default_value("8080".to_string()),
            ],
            capabilities: vec!["test-capability".to_string()],
            tool_permissions: ToolPermissions {
                allowed_tools: vec!["Read".to_string(), "Write".to_string()],
                denied_tools: vec![],
                file_access: FileAccessConfig::default(),
                network_access: NetworkAccessConfig::default(),
                process_access: ProcessAccessConfig::default(),
            },
            tags: vec!["test".to_string()],
            min_claude_version: Some("1.0.0".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(template_dir.join("template.json"), metadata_json)
            .await
            .unwrap();

        // Create agent-config.md
        let config_content = r#"# Agent Configuration for {{project_name}}

You are a test agent working on project "{{project_name}}".

## Configuration
- Port: {{port}}
- Capabilities: {{capabilities}}

## Instructions
Follow the test protocol for this project.
"#;
        fs::write(template_dir.join("agent-config.md"), config_content)
            .await
            .unwrap();

        // Create prompts directory with example prompts
        let prompts_dir = template_dir.join("prompts");
        fs::create_dir_all(&prompts_dir).await.unwrap();

        fs::write(
            prompts_dir.join("system.md"),
            "You are a system prompt for {{project_name}}.",
        )
        .await
        .unwrap();

        fs::write(
            prompts_dir.join("instructions.md"),
            "Follow these instructions for {{project_name}}:\n1. Be helpful\n2. Be accurate",
        )
        .await
        .unwrap();

        Ok(template_dir)
    }

    #[tokio::test]
    async fn test_filesystem_template_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = FilesystemTemplateManager::new(temp_dir.path());

        assert_eq!(manager.templates_directory, temp_dir.path());
    }

    #[tokio::test]
    async fn test_load_template() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "test-template")
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let template = manager.load_template("test-template").await.unwrap();

        assert_eq!(template.metadata.name, "test-template");
        assert_eq!(template.metadata.version, "1.0.0");
        assert_eq!(template.metadata.variables.len(), 2);
        assert!(template.config_template.contains("{{project_name}}"));
        assert_eq!(template.prompt_templates.len(), 2);
        assert!(template.prompt_templates.contains_key("system"));
        assert!(template.prompt_templates.contains_key("instructions"));
    }

    #[tokio::test]
    async fn test_load_nonexistent_template() {
        let temp_dir = TempDir::new().unwrap();
        let manager = FilesystemTemplateManager::new(temp_dir.path());

        let result = manager.load_template("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_list_templates() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "template-a").await.unwrap();
        create_test_template(&temp_dir, "template-b").await.unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let templates = manager.list_templates().await.unwrap();

        assert_eq!(templates.len(), 2);
        assert!(templates.contains(&"template-a".to_string()));
        assert!(templates.contains(&"template-b".to_string()));
    }

    #[tokio::test]
    async fn test_template_exists() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "existing-template")
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());

        assert!(manager.template_exists("existing-template").await.unwrap());
        assert!(!manager
            .template_exists("nonexistent-template")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_render_agent_config() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "test-template")
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let template = manager.load_template("test-template").await.unwrap();

        let mut variables = HashMap::new();
        variables.insert("project_name".to_string(), "MyProject".to_string());

        let rendered = manager
            .render_agent_config(&template, &variables)
            .await
            .unwrap();

        assert!(rendered.contains("MyProject"));
        assert!(rendered.contains("8080")); // Default port value
        assert!(rendered.contains("test-capability")); // Capabilities
    }

    #[tokio::test]
    async fn test_render_agent_config_missing_required_variable() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "test-template")
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let template = manager.load_template("test-template").await.unwrap();

        // Don't provide required project_name variable
        let variables = HashMap::new();

        let result = manager.render_agent_config(&template, &variables).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Validation { .. }));
    }

    #[tokio::test]
    async fn test_validate_template() {
        let temp_dir = TempDir::new().unwrap();
        create_test_template(&temp_dir, "test-template")
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let template = manager.load_template("test-template").await.unwrap();

        // Should pass validation
        assert!(manager.validate_template(&template).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_template_with_invalid_handlebars() {
        let temp_dir = TempDir::new().unwrap();
        let template_dir = temp_dir.path().join("invalid-template");
        fs::create_dir_all(&template_dir).await.unwrap();

        // Create valid template.json
        let metadata = AgentTemplateMetadata {
            name: "invalid-template".to_string(),
            description: "Invalid template".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            variables: Vec::new(),
            capabilities: Vec::new(),
            tool_permissions: ToolPermissions::default(),
            tags: Vec::new(),
            min_claude_version: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(template_dir.join("template.json"), metadata_json)
            .await
            .unwrap();

        // Create invalid agent-config.md with bad Handlebars syntax
        let invalid_config = "{{#if unclosed_block}}This is invalid";
        fs::write(template_dir.join("agent-config.md"), invalid_config)
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let result = manager.load_template("invalid-template").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Validation { .. }));
    }

    #[tokio::test]
    async fn test_list_templates_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let manager = FilesystemTemplateManager::new(temp_dir.path());

        let templates = manager.list_templates().await.unwrap();
        assert!(templates.is_empty());
    }

    #[tokio::test]
    async fn test_handlebars_helpers() {
        let temp_dir = TempDir::new().unwrap();
        let template_dir = temp_dir.path().join("helpers-template");
        fs::create_dir_all(&template_dir).await.unwrap();

        // Create template with helper usage
        let metadata = AgentTemplateMetadata {
            name: "helpers-template".to_string(),
            description: "Template with helpers".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            variables: vec![TemplateVariable::new(
                "name".to_string(),
                "Name to transform".to_string(),
                TemplateVariableType::String,
                true,
            )
            .unwrap()],
            capabilities: Vec::new(),
            tool_permissions: ToolPermissions::default(),
            tags: Vec::new(),
            min_claude_version: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(template_dir.join("template.json"), metadata_json)
            .await
            .unwrap();

        // Create config with helper usage
        let config_content = r#"# Template with helpers

Uppercase: {{upper name}}
Lowercase: {{lower name}}
"#;
        fs::write(template_dir.join("agent-config.md"), config_content)
            .await
            .unwrap();

        let manager = FilesystemTemplateManager::new(temp_dir.path());
        let template = manager.load_template("helpers-template").await.unwrap();

        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "TestName".to_string());

        let rendered = manager
            .render_agent_config(&template, &variables)
            .await
            .unwrap();

        assert!(rendered.contains("TESTNAME")); // upper helper
        assert!(rendered.contains("testname")); // lower helper
    }
}
