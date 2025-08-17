//! Prompt management functionality

use crate::{renderer::PromptRenderer, templates, Error, Result};
use std::{collections::HashMap, sync::Arc};
use tracing::{info, warn};
use uuid::Uuid;
use vibe_ensemble_core::{
    agent::AgentType,
    prompt::{PromptType, PromptVariable, SystemPrompt, VariableType},
};
use vibe_ensemble_storage::StorageManager;

/// Manager for system prompts and templates
pub struct PromptManager {
    storage: Arc<StorageManager>,
    renderer: PromptRenderer,
}

impl PromptManager {
    /// Create a new prompt manager
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self {
            storage,
            renderer: PromptRenderer::new(),
        }
    }

    /// Initialize the prompt manager with default prompts
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing prompt manager with default prompts");
        
        // Check if we already have prompts
        let existing_prompts = self.storage.prompts().list_active().await?;
        if !existing_prompts.is_empty() {
            info!("Found {} existing prompts, skipping initialization", existing_prompts.len());
            return Ok(());
        }

        // Create default prompts
        self.create_default_prompts().await?;
        
        info!("Prompt manager initialized successfully");
        Ok(())
    }

    /// Get a prompt suitable for an agent type
    pub async fn get_prompt_for_agent(&self, agent_type: &AgentType) -> Result<Option<SystemPrompt>> {
        let prompt_type = match agent_type {
            AgentType::Coordinator => PromptType::Coordinator,
            AgentType::Worker => PromptType::Worker,
        };

        let prompts = self.storage.prompts().find_by_type(&prompt_type).await?;
        
        // Return the most recent version
        Ok(prompts.into_iter().next())
    }

    /// Render a prompt with variables
    pub async fn render_prompt(
        &self,
        prompt_id: Uuid,
        variables: HashMap<String, String>,
    ) -> Result<String> {
        let prompt = self.storage.prompts().find_by_id(prompt_id).await?
            .ok_or_else(|| Error::PromptNotFound { id: prompt_id.to_string() })?;

        self.renderer.render(&prompt, variables).await
    }

    /// Create a new system prompt
    pub async fn create_prompt(&self, prompt: SystemPrompt) -> Result<()> {
        self.storage.prompts().create(&prompt).await?;
        info!("Created new system prompt: {}", prompt.name);
        Ok(())
    }

    /// Update an existing prompt
    pub async fn update_prompt(&self, prompt: SystemPrompt) -> Result<()> {
        self.storage.prompts().update(&prompt).await?;
        info!("Updated system prompt: {}", prompt.name);
        Ok(())
    }

    /// Deactivate a prompt
    pub async fn deactivate_prompt(&self, prompt_id: Uuid) -> Result<()> {
        let mut prompt = self.storage.prompts().find_by_id(prompt_id).await?
            .ok_or_else(|| Error::PromptNotFound { id: prompt_id.to_string() })?;

        prompt.deactivate();
        self.storage.prompts().update(&prompt).await?;
        
        info!("Deactivated system prompt: {}", prompt.name);
        Ok(())
    }

    /// List all active prompts
    pub async fn list_active_prompts(&self) -> Result<Vec<SystemPrompt>> {
        self.storage.prompts().list_active().await.map_err(Error::Storage)
    }

    /// Create default system prompts
    async fn create_default_prompts(&self) -> Result<()> {
        let system_id = Uuid::new_v4(); // Placeholder for system user

        // Coordinator prompt
        let coordinator_prompt = SystemPrompt::new(
            "Default Coordinator".to_string(),
            "Default system prompt for coordinator agents".to_string(),
            templates::COORDINATOR_TEMPLATE.to_string(),
            PromptType::Coordinator,
            system_id,
        );

        // Worker prompt
        let worker_prompt = SystemPrompt::new(
            "Default Worker".to_string(),
            "Default system prompt for worker agents".to_string(),
            templates::WORKER_TEMPLATE.to_string(),
            PromptType::Worker,
            system_id,
        );

        // Universal prompt
        let universal_prompt = SystemPrompt::new(
            "Universal Agent".to_string(),
            "Universal system prompt for all agent types".to_string(),
            templates::UNIVERSAL_TEMPLATE.to_string(),
            PromptType::Universal,
            system_id,
        );

        // Add variables to prompts
        let mut coordinator_with_vars = coordinator_prompt;
        coordinator_with_vars.add_variable(PromptVariable {
            name: "agent_name".to_string(),
            description: "Name of the agent".to_string(),
            variable_type: VariableType::String,
            default_value: Some("Coordinator".to_string()),
            required: true,
        });
        coordinator_with_vars.add_variable(PromptVariable {
            name: "team_size".to_string(),
            description: "Number of agents in the team".to_string(),
            variable_type: VariableType::Number,
            default_value: Some("1".to_string()),
            required: false,
        });

        let mut worker_with_vars = worker_prompt;
        worker_with_vars.add_variable(PromptVariable {
            name: "agent_name".to_string(),
            description: "Name of the agent".to_string(),
            variable_type: VariableType::String,
            default_value: Some("Worker".to_string()),
            required: true,
        });
        worker_with_vars.add_variable(PromptVariable {
            name: "specialization".to_string(),
            description: "Agent's area of specialization".to_string(),
            variable_type: VariableType::String,
            default_value: Some("General".to_string()),
            required: false,
        });

        // Create prompts in storage
        self.storage.prompts().create(&coordinator_with_vars).await?;
        self.storage.prompts().create(&worker_with_vars).await?;
        self.storage.prompts().create(&universal_prompt).await?;

        info!("Created default system prompts");
        Ok(())
    }
}