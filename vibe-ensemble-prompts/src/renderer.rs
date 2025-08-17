//! Prompt template rendering functionality

use crate::{Error, Result};
use std::collections::HashMap;
use tracing::warn;
use vibe_ensemble_core::prompt::{RenderedPrompt, SystemPrompt, VariableType};

/// Renders prompt templates with variable substitution
pub struct PromptRenderer;

impl PromptRenderer {
    /// Create a new prompt renderer
    pub fn new() -> Self {
        Self
    }

    /// Render a prompt template with provided variables
    pub async fn render(
        &self,
        prompt: &SystemPrompt,
        variables: HashMap<String, String>,
    ) -> Result<String> {
        // Validate required variables
        self.validate_variables(prompt, &variables)?;

        // Start with the template
        let mut rendered = prompt.template.clone();

        // Substitute variables
        for variable in &prompt.variables {
            let value = variables.get(&variable.name)
                .or(variable.default_value.as_ref())
                .ok_or_else(|| Error::MissingVariable {
                    name: variable.name.clone(),
                })?;

            // Validate variable type
            self.validate_variable_type(variable, value)?;

            // Replace all occurrences of {{variable_name}} with the value
            let placeholder = format!("{{{{{}}}}}", variable.name);
            rendered = rendered.replace(&placeholder, value);
        }

        // Check for unreplaced variables (this indicates template issues)
        if rendered.contains("{{") && rendered.contains("}}") {
            warn!("Template contains unreplaced variables: {}", rendered);
        }

        Ok(rendered)
    }

    /// Create a rendered prompt record
    pub async fn create_rendered_prompt(
        &self,
        prompt: &SystemPrompt,
        variables: HashMap<String, String>,
    ) -> Result<RenderedPrompt> {
        let content = self.render(prompt, variables.clone()).await?;
        
        Ok(RenderedPrompt {
            prompt_id: prompt.id,
            content,
            rendered_at: chrono::Utc::now(),
            variables_used: variables,
        })
    }

    /// Validate that all required variables are provided
    fn validate_variables(
        &self,
        prompt: &SystemPrompt,
        variables: &HashMap<String, String>,
    ) -> Result<()> {
        for variable in &prompt.variables {
            if variable.required {
                if !variables.contains_key(&variable.name) && variable.default_value.is_none() {
                    return Err(Error::MissingVariable {
                        name: variable.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Validate that a variable value matches its expected type
    fn validate_variable_type(
        &self,
        variable: &vibe_ensemble_core::prompt::PromptVariable,
        value: &str,
    ) -> Result<()> {
        match variable.variable_type {
            VariableType::String => Ok(()),
            VariableType::Number => {
                value.parse::<f64>()
                    .map_err(|_| Error::InvalidTemplate(
                        format!("Variable '{}' must be a number, got '{}'", variable.name, value)
                    ))?;
                Ok(())
            }
            VariableType::Boolean => {
                value.parse::<bool>()
                    .map_err(|_| Error::InvalidTemplate(
                        format!("Variable '{}' must be a boolean, got '{}'", variable.name, value)
                    ))?;
                Ok(())
            }
            VariableType::AgentId => {
                // Validate UUID format
                uuid::Uuid::parse_str(value)
                    .map_err(|_| Error::InvalidTemplate(
                        format!("Variable '{}' must be a valid UUID, got '{}'", variable.name, value)
                    ))?;
                Ok(())
            }
            VariableType::IssueId => {
                // Validate UUID format
                uuid::Uuid::parse_str(value)
                    .map_err(|_| Error::InvalidTemplate(
                        format!("Variable '{}' must be a valid UUID, got '{}'", variable.name, value)
                    ))?;
                Ok(())
            }
            VariableType::Timestamp => {
                // Validate ISO 8601 timestamp format
                chrono::DateTime::parse_from_rfc3339(value)
                    .map_err(|_| Error::InvalidTemplate(
                        format!("Variable '{}' must be a valid ISO 8601 timestamp, got '{}'", variable.name, value)
                    ))?;
                Ok(())
            }
        }
    }
}

impl Default for PromptRenderer {
    fn default() -> Self {
        Self::new()
    }
}