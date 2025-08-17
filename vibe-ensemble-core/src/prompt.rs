//! System prompt domain model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl SystemPrompt {
    /// Create a new system prompt
    pub fn new(
        name: String,
        description: String,
        template: String,
        prompt_type: PromptType,
        created_by: Uuid,
    ) -> Self {
        let now = Utc::now();
        Self {
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
        }
    }

    /// Add a variable to the prompt template
    pub fn add_variable(&mut self, variable: PromptVariable) {
        self.variables.push(variable);
        self.updated_at = Utc::now();
    }

    /// Update the prompt template and increment version
    pub fn update_template(&mut self, template: String) {
        self.template = template;
        self.updated_at = Utc::now();
        self.version += 1;
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

        match (&self.prompt_type, agent_type) {
            (PromptType::Universal, _) => true,
            (PromptType::Coordinator, crate::agent::AgentType::Coordinator) => true,
            (PromptType::Worker, crate::agent::AgentType::Worker) => true,
            _ => false,
        }
    }
}