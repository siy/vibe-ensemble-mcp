//! System prompt domain model and related types
//!
//! This module provides the core system prompt and agent template models
//! for managing versioned prompts and agent configurations in the
//! Vibe Ensemble system.
//!
//! # Examples
//!
//! Creating a new system prompt:
//!
//! ```rust
//! use vibe_ensemble_core::prompt::*;
//! use uuid::Uuid;
//!
//! let prompt = SystemPrompt::builder()
//!     .name("coordinator-v1")
//!     .description("Main coordinator prompt for task distribution")
//!     .template("You are a task coordinator. Your role is to {{role_description}}...")
//!     .prompt_type(PromptType::Coordinator)
//!     .created_by(Uuid::new_v4())
//!     .variable(PromptVariable::new(
//!         "role_description".to_string(),
//!         "Description of the coordinator role".to_string(),
//!         VariableType::String,
//!         true,
//!     ).unwrap())
//!     .build()
//!     .unwrap();
//! ```
//!
//! Creating an agent template:
//!
//! ```rust
//! use vibe_ensemble_core::prompt::*;
//! use vibe_ensemble_core::agent::AgentType;
//! use uuid::Uuid;
//!
//! let template = AgentTemplate::builder()
//!     .name("code-review-specialist")
//!     .description("Specialized agent for code review tasks")
//!     .agent_type(AgentType::Worker)
//!     .created_by(Uuid::new_v4())
//!     .capability("code-review")
//!     .capability("static-analysis")
//!     .workflow_step(WorkflowStep::new(
//!         "analyze".to_string(),
//!         "Analyze Code".to_string(),
//!         "Analyze the code for issues".to_string(),
//!         1,
//!     ).unwrap())
//!     .workflow_step(WorkflowStep::new(
//!         "report".to_string(),
//!         "Generate Report".to_string(),
//!         "Generate review report".to_string(),
//!         2,
//!     ).unwrap())
//!     .build()
//!     .unwrap();
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{Error, Result};

/// Represents a system prompt template
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemPrompt {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub template: String,
    pub prompt_type: PromptType,
    pub variables: Vec<PromptVariable>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub is_active: bool,
}

/// Type of system prompt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PromptType {
    Coordinator,
    Worker,
    Specialist { domain: String },
    Universal,
}

/// Variable that can be substituted in a prompt template
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptVariable {
    pub name: String,
    pub description: String,
    pub variable_type: VariableType,
    pub default_value: Option<String>,
    pub required: bool,
}

/// Type of prompt variable
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    AgentId,
    IssueId,
    Timestamp,
}

/// A rendered prompt ready for use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedPrompt {
    pub prompt_id: Uuid,
    pub content: String,
    pub rendered_at: DateTime<Utc>,
    pub variables_used: std::collections::HashMap<String, String>,
}

/// Claude Code agent template configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub agent_type: crate::agent::AgentType,
    pub capabilities: Vec<String>,
    pub system_prompt_id: Option<Uuid>,
    pub workflow_steps: Vec<WorkflowStep>,
    pub configuration_params: std::collections::HashMap<String, String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub is_active: bool,
}

/// Workflow step for agent orchestration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub order: u32,
    pub conditions: Vec<StepCondition>,
    pub timeout_seconds: Option<u64>,
    pub retry_policy: Option<crate::config::RetryPolicy>,
}

/// Condition for workflow step execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepCondition {
    pub condition_type: ConditionType,
    pub value: String,
}

/// Type of step condition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConditionType {
    PreviousStepSuccess,
    PreviousStepFailure,
    VariableEquals { variable: String },
    CapabilityRequired,
    Custom { expression: String },
}

impl SystemPrompt {
    /// Create a new system prompt with validation
    pub fn new(
        name: String,
        description: String,
        template: String,
        prompt_type: PromptType,
        created_by: Uuid,
    ) -> Result<Self> {
        Self::validate_name(&name)?;
        Self::validate_template(&template)?;
        
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            template,
            prompt_type,
            variables: Vec::new(),
            created_by,
            created_at: now,
            updated_at: now,
            version: 1,
            is_active: true,
        })
    }

    /// Create a builder for constructing a SystemPrompt
    pub fn builder() -> SystemPromptBuilder {
        SystemPromptBuilder::new()
    }

    /// Validate prompt name
    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "System prompt name cannot be empty".to_string(),
            });
        }
        if name.len() > 100 {
            return Err(Error::Validation {
                message: "System prompt name cannot exceed 100 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Validate prompt template
    fn validate_template(template: &str) -> Result<()> {
        if template.trim().is_empty() {
            return Err(Error::Validation {
                message: "System prompt template cannot be empty".to_string(),
            });
        }
        if template.len() > 50000 {
            return Err(Error::Validation {
                message: "System prompt template cannot exceed 50000 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Add a variable to the prompt template
    pub fn add_variable(&mut self, variable: PromptVariable) -> Result<()> {
        // Check for duplicate variable names
        if self.variables.iter().any(|v| v.name == variable.name) {
            return Err(Error::Validation {
                message: format!("Variable '{}' already exists in this prompt", variable.name),
            });
        }
        self.variables.push(variable);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a variable from the prompt template
    pub fn remove_variable(&mut self, variable_name: &str) {
        if let Some(pos) = self.variables.iter().position(|v| v.name == variable_name) {
            self.variables.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&PromptVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// Update the prompt template and increment version
    pub fn update_template(&mut self, template: String) -> Result<()> {
        Self::validate_template(&template)?;
        self.template = template;
        self.updated_at = Utc::now();
        self.version += 1;
        Ok(())
    }

    /// Deactivate the prompt
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.updated_at = Utc::now();
    }

    /// Check if the prompt is suitable for a given agent type
    pub fn is_suitable_for(&self, agent_type: &crate::agent::AgentType) -> bool {
        if !self.is_active {
            return false;
        }

        matches!((&self.prompt_type, agent_type), 
            (PromptType::Universal, _) | 
            (PromptType::Coordinator, crate::agent::AgentType::Coordinator) | 
            (PromptType::Worker, crate::agent::AgentType::Worker))
    }

    /// Render the prompt with provided variable values
    pub fn render(&self, variables: &std::collections::HashMap<String, String>) -> Result<RenderedPrompt> {
        let mut content = self.template.clone();
        let mut variables_used = std::collections::HashMap::new();
        
        // Check for required variables
        for prompt_var in &self.variables {
            if prompt_var.required && !variables.contains_key(&prompt_var.name) && prompt_var.default_value.is_none() {
                return Err(Error::Validation {
                    message: format!("Required variable '{}' not provided", prompt_var.name),
                });
            }
        }
        
        // Replace variables in template
        for prompt_var in &self.variables {
            let placeholder = format!("{{{{{}}}}}", prompt_var.name);
            if content.contains(&placeholder) {
                let value = variables.get(&prompt_var.name)
                    .or(prompt_var.default_value.as_ref())
                    .ok_or_else(|| Error::Validation {
                        message: format!("No value provided for variable '{}'", prompt_var.name),
                    })?;
                
                content = content.replace(&placeholder, value);
                variables_used.insert(prompt_var.name.clone(), value.clone());
            }
        }
        
        Ok(RenderedPrompt {
            prompt_id: self.id,
            content,
            rendered_at: Utc::now(),
            variables_used,
        })
    }

    /// Get the age of the prompt in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }

    /// Get the time since last update in seconds
    pub fn time_since_update_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.updated_at).num_seconds()
    }
}

impl PromptVariable {
    /// Create a new prompt variable with validation
    pub fn new(
        name: String,
        description: String,
        variable_type: VariableType,
        required: bool,
    ) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Variable name cannot be empty".to_string(),
            });
        }
        if name.len() > 50 {
            return Err(Error::Validation {
                message: "Variable name cannot exceed 50 characters".to_string(),
            });
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(Error::Validation {
                message: "Variable name can only contain alphanumeric characters and underscores".to_string(),
            });
        }
        
        Ok(Self {
            name,
            description,
            variable_type,
            default_value: None,
            required,
        })
    }

    /// Set the default value for the variable
    pub fn with_default_value(mut self, default_value: String) -> Self {
        self.default_value = Some(default_value);
        self
    }
}

impl AgentTemplate {
    /// Create a new agent template with validation
    pub fn new(
        name: String,
        description: String,
        agent_type: crate::agent::AgentType,
        created_by: Uuid,
    ) -> Result<Self> {
        Self::validate_name(&name)?;
        
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            agent_type,
            capabilities: Vec::new(),
            system_prompt_id: None,
            workflow_steps: Vec::new(),
            configuration_params: std::collections::HashMap::new(),
            created_by,
            created_at: now,
            updated_at: now,
            version: 1,
            is_active: true,
        })
    }

    /// Create a builder for constructing an AgentTemplate
    pub fn builder() -> AgentTemplateBuilder {
        AgentTemplateBuilder::new()
    }

    /// Validate template name
    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Agent template name cannot be empty".to_string(),
            });
        }
        if name.len() > 100 {
            return Err(Error::Validation {
                message: "Agent template name cannot exceed 100 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Add a capability to the template
    pub fn add_capability(&mut self, capability: String) -> Result<()> {
        if capability.trim().is_empty() {
            return Err(Error::Validation {
                message: "Capability cannot be empty".to_string(),
            });
        }
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
            self.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Remove a capability from the template
    pub fn remove_capability(&mut self, capability: &str) {
        if let Some(pos) = self.capabilities.iter().position(|c| c == capability) {
            self.capabilities.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Check if the template has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }

    /// Add a workflow step
    pub fn add_workflow_step(&mut self, step: WorkflowStep) -> Result<()> {
        // Check for duplicate step IDs
        if self.workflow_steps.iter().any(|s| s.id == step.id) {
            return Err(Error::Validation {
                message: format!("Workflow step '{}' already exists", step.id),
            });
        }
        self.workflow_steps.push(step);
        // Sort steps by order
        self.workflow_steps.sort_by(|a, b| a.order.cmp(&b.order));
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a workflow step
    pub fn remove_workflow_step(&mut self, step_id: &str) {
        if let Some(pos) = self.workflow_steps.iter().position(|s| s.id == step_id) {
            self.workflow_steps.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Get a workflow step by ID
    pub fn get_workflow_step(&self, step_id: &str) -> Option<&WorkflowStep> {
        self.workflow_steps.iter().find(|s| s.id == step_id)
    }

    /// Set system prompt ID
    pub fn set_system_prompt_id(&mut self, prompt_id: Option<Uuid>) {
        if self.system_prompt_id != prompt_id {
            self.system_prompt_id = prompt_id;
            self.updated_at = Utc::now();
        }
    }

    /// Add a configuration parameter
    pub fn add_config_param(&mut self, key: String, value: String) -> Result<()> {
        if key.trim().is_empty() {
            return Err(Error::Validation {
                message: "Configuration parameter key cannot be empty".to_string(),
            });
        }
        self.configuration_params.insert(key, value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a configuration parameter
    pub fn remove_config_param(&mut self, key: &str) {
        if self.configuration_params.remove(key).is_some() {
            self.updated_at = Utc::now();
        }
    }

    /// Get a configuration parameter
    pub fn get_config_param(&self, key: &str) -> Option<&String> {
        self.configuration_params.get(key)
    }

    /// Activate the template
    pub fn activate(&mut self) {
        if !self.is_active {
            self.is_active = true;
            self.updated_at = Utc::now();
        }
    }

    /// Deactivate the template
    pub fn deactivate(&mut self) {
        if self.is_active {
            self.is_active = false;
            self.updated_at = Utc::now();
        }
    }

    /// Get the age of the template in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }
}

impl WorkflowStep {
    /// Create a new workflow step with validation
    pub fn new(
        id: String,
        name: String,
        description: String,
        order: u32,
    ) -> Result<Self> {
        if id.trim().is_empty() {
            return Err(Error::Validation {
                message: "Workflow step ID cannot be empty".to_string(),
            });
        }
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Workflow step name cannot be empty".to_string(),
            });
        }
        
        Ok(Self {
            id,
            name,
            description,
            order,
            conditions: Vec::new(),
            timeout_seconds: None,
            retry_policy: None,
        })
    }

    /// Add a condition to the step
    pub fn add_condition(&mut self, condition: StepCondition) {
        self.conditions.push(condition);
    }

    /// Set timeout for the step
    pub fn set_timeout(&mut self, timeout_seconds: u64) {
        self.timeout_seconds = Some(timeout_seconds);
    }

    /// Set retry policy for the step
    pub fn set_retry_policy(&mut self, policy: crate::config::RetryPolicy) {
        self.retry_policy = Some(policy);
    }
}

impl StepCondition {
    /// Create a new step condition
    pub fn new(condition_type: ConditionType, value: String) -> Self {
        Self {
            condition_type,
            value,
        }
    }
}

/// Builder for constructing SystemPrompt instances with validation
#[derive(Debug, Clone)]
pub struct SystemPromptBuilder {
    name: Option<String>,
    description: Option<String>,
    template: Option<String>,
    prompt_type: Option<PromptType>,
    created_by: Option<Uuid>,
    variables: Vec<PromptVariable>,
}

impl SystemPromptBuilder {
    /// Create a new system prompt builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            template: None,
            prompt_type: None,
            created_by: None,
            variables: Vec::new(),
        }
    }

    /// Set the prompt name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the prompt description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the prompt template
    pub fn template<S: Into<String>>(mut self, template: S) -> Self {
        self.template = Some(template.into());
        self
    }

    /// Set the prompt type
    pub fn prompt_type(mut self, prompt_type: PromptType) -> Self {
        self.prompt_type = Some(prompt_type);
        self
    }

    /// Set the creator ID
    pub fn created_by(mut self, created_by: Uuid) -> Self {
        self.created_by = Some(created_by);
        self
    }

    /// Add a variable
    pub fn variable(mut self, variable: PromptVariable) -> Self {
        self.variables.push(variable);
        self
    }

    /// Build the SystemPrompt instance
    pub fn build(self) -> Result<SystemPrompt> {
        let name = self.name.ok_or_else(|| Error::Validation {
            message: "System prompt name is required".to_string(),
        })?;
        let description = self.description.ok_or_else(|| Error::Validation {
            message: "System prompt description is required".to_string(),
        })?;
        let template = self.template.ok_or_else(|| Error::Validation {
            message: "System prompt template is required".to_string(),
        })?;
        let prompt_type = self.prompt_type.ok_or_else(|| Error::Validation {
            message: "System prompt type is required".to_string(),
        })?;
        let created_by = self.created_by.ok_or_else(|| Error::Validation {
            message: "Creator ID is required".to_string(),
        })?;

        let mut prompt = SystemPrompt::new(name, description, template, prompt_type, created_by)?;
        
        // Add variables
        for variable in self.variables {
            prompt.add_variable(variable)?;
        }
        
        Ok(prompt)
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing AgentTemplate instances with validation
#[derive(Debug, Clone)]
pub struct AgentTemplateBuilder {
    name: Option<String>,
    description: Option<String>,
    agent_type: Option<crate::agent::AgentType>,
    created_by: Option<Uuid>,
    capabilities: Vec<String>,
    system_prompt_id: Option<Uuid>,
    workflow_steps: Vec<WorkflowStep>,
    configuration_params: std::collections::HashMap<String, String>,
}

impl AgentTemplateBuilder {
    /// Create a new agent template builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            agent_type: None,
            created_by: None,
            capabilities: Vec::new(),
            system_prompt_id: None,
            workflow_steps: Vec::new(),
            configuration_params: std::collections::HashMap::new(),
        }
    }

    /// Set the template name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the template description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the agent type
    pub fn agent_type(mut self, agent_type: crate::agent::AgentType) -> Self {
        self.agent_type = Some(agent_type);
        self
    }

    /// Set the creator ID
    pub fn created_by(mut self, created_by: Uuid) -> Self {
        self.created_by = Some(created_by);
        self
    }

    /// Add a capability
    pub fn capability<S: Into<String>>(mut self, capability: S) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Add multiple capabilities
    pub fn capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.capabilities.extend(capabilities.into_iter().map(|c| c.into()));
        self
    }

    /// Set the system prompt ID
    pub fn system_prompt_id(mut self, prompt_id: Uuid) -> Self {
        self.system_prompt_id = Some(prompt_id);
        self
    }

    /// Add a workflow step
    pub fn workflow_step(mut self, step: WorkflowStep) -> Self {
        self.workflow_steps.push(step);
        self
    }

    /// Add a configuration parameter
    pub fn config_param<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.configuration_params.insert(key.into(), value.into());
        self
    }

    /// Build the AgentTemplate instance
    pub fn build(self) -> Result<AgentTemplate> {
        let name = self.name.ok_or_else(|| Error::Validation {
            message: "Agent template name is required".to_string(),
        })?;
        let description = self.description.ok_or_else(|| Error::Validation {
            message: "Agent template description is required".to_string(),
        })?;
        let agent_type = self.agent_type.ok_or_else(|| Error::Validation {
            message: "Agent type is required".to_string(),
        })?;
        let created_by = self.created_by.ok_or_else(|| Error::Validation {
            message: "Creator ID is required".to_string(),
        })?;

        let mut template = AgentTemplate::new(name, description, agent_type, created_by)?;
        
        // Add capabilities
        for capability in self.capabilities {
            template.add_capability(capability)?;
        }
        
        // Set system prompt ID
        template.set_system_prompt_id(self.system_prompt_id);
        
        // Add workflow steps
        for step in self.workflow_steps {
            template.add_workflow_step(step)?;
        }
        
        // Add configuration parameters
        for (key, value) in self.configuration_params {
            template.add_config_param(key, value)?;
        }
        
        Ok(template)
    }
}

impl Default for AgentTemplateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_creation_with_builder() {
        let creator_id = Uuid::new_v4();
        
        let variable = PromptVariable::new(
            "task_type".to_string(),
            "Type of task to perform".to_string(),
            VariableType::String,
            true,
        ).unwrap().with_default_value("general".to_string());
        
        let prompt = SystemPrompt::builder()
            .name("test-coordinator")
            .description("Test coordinator prompt")
            .template("You are a {{task_type}} coordinator. Your role is to manage tasks.")
            .prompt_type(PromptType::Coordinator)
            .created_by(creator_id)
            .variable(variable)
            .build()
            .unwrap();

        assert_eq!(prompt.name, "test-coordinator");
        assert_eq!(prompt.prompt_type, PromptType::Coordinator);
        assert_eq!(prompt.created_by, creator_id);
        assert_eq!(prompt.variables.len(), 1);
        assert!(prompt.is_active);
        assert_eq!(prompt.version, 1);
    }

    #[test]
    fn test_system_prompt_validation() {
        let creator_id = Uuid::new_v4();
        
        // Empty name should fail
        let result = SystemPrompt::builder()
            .name("")
            .description("Test description")
            .template("Test template")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .build();
        assert!(result.is_err());

        // Too long name should fail
        let long_name = "a".repeat(101);
        let result = SystemPrompt::builder()
            .name(long_name)
            .description("Test description")
            .template("Test template")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .build();
        assert!(result.is_err());

        // Empty template should fail
        let result = SystemPrompt::builder()
            .name("test-prompt")
            .description("Test description")
            .template("")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_variable_validation() {
        // Empty name should fail
        let result = PromptVariable::new(
            "".to_string(),
            "Test description".to_string(),
            VariableType::String,
            false,
        );
        assert!(result.is_err());

        // Too long name should fail
        let long_name = "a".repeat(51);
        let result = PromptVariable::new(
            long_name,
            "Test description".to_string(),
            VariableType::String,
            false,
        );
        assert!(result.is_err());

        // Invalid characters should fail
        let result = PromptVariable::new(
            "test-var".to_string(),
            "Test description".to_string(),
            VariableType::String,
            false,
        );
        assert!(result.is_err());

        // Valid variable should succeed
        let result = PromptVariable::new(
            "test_var".to_string(),
            "Test description".to_string(),
            VariableType::String,
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_prompt_rendering() {
        let creator_id = Uuid::new_v4();
        
        let variable1 = PromptVariable::new(
            "role".to_string(),
            "Role description".to_string(),
            VariableType::String,
            true,
        ).unwrap();
        
        let variable2 = PromptVariable::new(
            "optional_param".to_string(),
            "Optional parameter".to_string(),
            VariableType::String,
            false,
        ).unwrap().with_default_value("default_value".to_string());
        
        let prompt = SystemPrompt::builder()
            .name("test-prompt")
            .description("Test prompt")
            .template("You are a {{role}} agent. Optional: {{optional_param}}")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .variable(variable1)
            .variable(variable2)
            .build()
            .unwrap();

        // Test successful rendering
        let mut variables = std::collections::HashMap::new();
        variables.insert("role".to_string(), "coordinator".to_string());
        
        let rendered = prompt.render(&variables).unwrap();
        assert!(rendered.content.contains("coordinator"));
        assert!(rendered.content.contains("default_value"));
        assert_eq!(rendered.variables_used.len(), 2);

        // Test missing required variable
        let empty_variables = std::collections::HashMap::new();
        let result = prompt.render(&empty_variables);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_variable_operations() {
        let creator_id = Uuid::new_v4();
        
        let mut prompt = SystemPrompt::builder()
            .name("test-prompt")
            .description("Test prompt")
            .template("Test template")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        let variable = PromptVariable::new(
            "test_var".to_string(),
            "Test variable".to_string(),
            VariableType::String,
            false,
        ).unwrap();

        // Add variable
        prompt.add_variable(variable.clone()).unwrap();
        assert_eq!(prompt.variables.len(), 1);
        assert!(prompt.get_variable("test_var").is_some());

        // Adding duplicate variable should fail
        let result = prompt.add_variable(variable);
        assert!(result.is_err());

        // Remove variable
        prompt.remove_variable("test_var");
        assert_eq!(prompt.variables.len(), 0);
        assert!(prompt.get_variable("test_var").is_none());
    }

    #[test]
    fn test_prompt_suitability() {
        let creator_id = Uuid::new_v4();
        
        // Test coordinator prompt
        let coordinator_prompt = SystemPrompt::builder()
            .name("coordinator-prompt")
            .description("Coordinator prompt")
            .template("Test template")
            .prompt_type(PromptType::Coordinator)
            .created_by(creator_id)
            .build()
            .unwrap();

        assert!(coordinator_prompt.is_suitable_for(&crate::agent::AgentType::Coordinator));
        assert!(!coordinator_prompt.is_suitable_for(&crate::agent::AgentType::Worker));

        // Test universal prompt
        let universal_prompt = SystemPrompt::builder()
            .name("universal-prompt")
            .description("Universal prompt")
            .template("Test template")
            .prompt_type(PromptType::Universal)
            .created_by(creator_id)
            .build()
            .unwrap();

        assert!(universal_prompt.is_suitable_for(&crate::agent::AgentType::Coordinator));
        assert!(universal_prompt.is_suitable_for(&crate::agent::AgentType::Worker));

        // Test inactive prompt
        let mut inactive_prompt = universal_prompt.clone();
        inactive_prompt.deactivate();
        assert!(!inactive_prompt.is_suitable_for(&crate::agent::AgentType::Worker));
    }

    #[test]
    fn test_agent_template_creation() {
        let creator_id = Uuid::new_v4();
        let prompt_id = Uuid::new_v4();
        
        let step = WorkflowStep::new(
            "analyze".to_string(),
            "Analyze Code".to_string(),
            "Perform static analysis on code".to_string(),
            1,
        ).unwrap();
        
        let template = AgentTemplate::builder()
            .name("code-reviewer")
            .description("Specialized code review agent")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .capability("code-review")
            .capability("static-analysis")
            .system_prompt_id(prompt_id)
            .workflow_step(step)
            .config_param("max_files", "50")
            .build()
            .unwrap();

        assert_eq!(template.name, "code-reviewer");
        assert_eq!(template.agent_type, crate::agent::AgentType::Worker);
        assert_eq!(template.capabilities.len(), 2);
        assert!(template.has_capability("code-review"));
        assert_eq!(template.system_prompt_id, Some(prompt_id));
        assert_eq!(template.workflow_steps.len(), 1);
        assert_eq!(template.configuration_params.len(), 1);
        assert!(template.is_active);
    }

    #[test]
    fn test_agent_template_validation() {
        let creator_id = Uuid::new_v4();
        
        // Empty name should fail
        let result = AgentTemplate::builder()
            .name("")
            .description("Test description")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .build();
        assert!(result.is_err());

        // Missing required fields should fail
        let result = AgentTemplate::builder()
            .name("test-template")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_template_capability_operations() {
        let creator_id = Uuid::new_v4();
        
        let mut template = AgentTemplate::builder()
            .name("test-template")
            .description("Test template")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .capability("initial-capability")
            .build()
            .unwrap();

        assert!(template.has_capability("initial-capability"));
        
        // Add capability
        template.add_capability("new-capability".to_string()).unwrap();
        assert!(template.has_capability("new-capability"));
        
        // Adding duplicate capability should not error
        template.add_capability("new-capability".to_string()).unwrap();
        assert_eq!(template.capabilities.len(), 2); // Should still be 2
        
        // Empty capability should fail
        let result = template.add_capability("".to_string());
        assert!(result.is_err());
        
        // Remove capability
        template.remove_capability("new-capability");
        assert!(!template.has_capability("new-capability"));
    }

    #[test]
    fn test_workflow_step_operations() {
        let creator_id = Uuid::new_v4();
        
        let mut template = AgentTemplate::builder()
            .name("test-template")
            .description("Test template")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        let step1 = WorkflowStep::new(
            "step1".to_string(),
            "First Step".to_string(),
            "First step description".to_string(),
            1,
        ).unwrap();
        
        let step2 = WorkflowStep::new(
            "step2".to_string(),
            "Second Step".to_string(),
            "Second step description".to_string(),
            2,
        ).unwrap();

        // Add workflow steps
        template.add_workflow_step(step1).unwrap();
        template.add_workflow_step(step2).unwrap();
        assert_eq!(template.workflow_steps.len(), 2);
        
        // Steps should be sorted by order
        assert_eq!(template.workflow_steps[0].id, "step1");
        assert_eq!(template.workflow_steps[1].id, "step2");
        
        // Get step
        assert!(template.get_workflow_step("step1").is_some());
        assert!(template.get_workflow_step("nonexistent").is_none());
        
        // Adding duplicate step should fail
        let duplicate_step = WorkflowStep::new(
            "step1".to_string(),
            "Duplicate Step".to_string(),
            "Duplicate description".to_string(),
            3,
        ).unwrap();
        let result = template.add_workflow_step(duplicate_step);
        assert!(result.is_err());
        
        // Remove step
        template.remove_workflow_step("step1");
        assert_eq!(template.workflow_steps.len(), 1);
        assert!(template.get_workflow_step("step1").is_none());
    }

    #[test]
    fn test_workflow_step_validation() {
        // Empty ID should fail
        let result = WorkflowStep::new(
            "".to_string(),
            "Test Step".to_string(),
            "Test description".to_string(),
            1,
        );
        assert!(result.is_err());

        // Empty name should fail
        let result = WorkflowStep::new(
            "step1".to_string(),
            "".to_string(),
            "Test description".to_string(),
            1,
        );
        assert!(result.is_err());

        // Valid step should succeed
        let result = WorkflowStep::new(
            "step1".to_string(),
            "Test Step".to_string(),
            "Test description".to_string(),
            1,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_workflow_step_conditions() {
        let mut step = WorkflowStep::new(
            "test_step".to_string(),
            "Test Step".to_string(),
            "Test description".to_string(),
            1,
        ).unwrap();

        let condition = StepCondition::new(
            ConditionType::PreviousStepSuccess,
            "step_id".to_string(),
        );

        step.add_condition(condition);
        assert_eq!(step.conditions.len(), 1);
    }

    #[test]
    fn test_configuration_parameter_operations() {
        let creator_id = Uuid::new_v4();
        
        let mut template = AgentTemplate::builder()
            .name("test-template")
            .description("Test template")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        // Add config parameter
        template.add_config_param("timeout".to_string(), "300".to_string()).unwrap();
        assert_eq!(template.get_config_param("timeout"), Some(&"300".to_string()));
        
        // Empty key should fail
        let result = template.add_config_param("".to_string(), "value".to_string());
        assert!(result.is_err());
        
        // Remove config parameter
        template.remove_config_param("timeout");
        assert!(template.get_config_param("timeout").is_none());
    }

    #[test]
    fn test_template_lifecycle() {
        let creator_id = Uuid::new_v4();
        
        let mut template = AgentTemplate::builder()
            .name("test-template")
            .description("Test template")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        assert!(template.is_active);
        
        template.deactivate();
        assert!(!template.is_active);
        
        template.activate();
        assert!(template.is_active);

        let age = template.age_seconds();
        assert!(age >= 0);
    }

    #[test]
    fn test_prompt_lifecycle() {
        let creator_id = Uuid::new_v4();
        
        let mut prompt = SystemPrompt::builder()
            .name("test-prompt")
            .description("Test prompt")
            .template("Test template")
            .prompt_type(PromptType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        assert!(prompt.is_active);
        
        prompt.deactivate();
        assert!(!prompt.is_active);

        let age = prompt.age_seconds();
        let time_since_update = prompt.time_since_update_seconds();
        assert!(age >= 0);
        assert!(time_since_update >= 0);

        // Test template update
        let initial_version = prompt.version;
        prompt.update_template("Updated template".to_string()).unwrap();
        assert_eq!(prompt.version, initial_version + 1);
        assert_eq!(prompt.template, "Updated template");
    }

    #[test]
    fn test_system_prompt_id_operations() {
        let creator_id = Uuid::new_v4();
        let prompt_id1 = Uuid::new_v4();
        let prompt_id2 = Uuid::new_v4();
        
        let mut template = AgentTemplate::builder()
            .name("test-template")
            .description("Test template")
            .agent_type(crate::agent::AgentType::Worker)
            .created_by(creator_id)
            .build()
            .unwrap();

        assert_eq!(template.system_prompt_id, None);
        
        template.set_system_prompt_id(Some(prompt_id1));
        assert_eq!(template.system_prompt_id, Some(prompt_id1));
        
        template.set_system_prompt_id(Some(prompt_id2));
        assert_eq!(template.system_prompt_id, Some(prompt_id2));
        
        template.set_system_prompt_id(None);
        assert_eq!(template.system_prompt_id, None);
    }
}