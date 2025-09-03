//! Prompt management functionality

use crate::{renderer::PromptRenderer, templates, Error, Result};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;
use vibe_ensemble_core::{
    agent::AgentType,
    prompt::{
        ExperimentResults, ExperimentStatus, FeedbackType, MetricType, PromptCacheEntry,
        PromptExperiment, PromptFeedback, PromptMetrics, PromptType, PromptVariable, SystemPrompt,
        TestRecommendation, VariableType,
    },
};
use vibe_ensemble_storage::StorageManager;

/// Manager for system prompts and templates with advanced features
pub struct PromptManager {
    storage: Arc<StorageManager>,
    renderer: PromptRenderer,
    cache: Arc<RwLock<HashMap<String, PromptCacheEntry>>>,
    cache_ttl: Duration,
}

impl PromptManager {
    /// Create a new prompt manager
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self {
            storage,
            renderer: PromptRenderer::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(3600), // 1 hour default TTL
        }
    }

    /// Create a new prompt manager with custom cache TTL
    pub fn with_cache_ttl(storage: Arc<StorageManager>, cache_ttl: Duration) -> Self {
        Self {
            storage,
            renderer: PromptRenderer::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    /// Initialize the prompt manager with default prompts
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing prompt manager with default prompts");

        // Check if we already have prompts
        let existing_prompts = self.storage.prompts().list_active().await?;
        if !existing_prompts.is_empty() {
            info!(
                "Found {} existing prompts, skipping initialization",
                existing_prompts.len()
            );
            return Ok(());
        }

        // Create default prompts
        self.create_default_prompts().await?;

        info!("Prompt manager initialized successfully");
        Ok(())
    }

    /// Get a prompt suitable for an agent type
    pub async fn get_prompt_for_agent(
        &self,
        agent_type: &AgentType,
    ) -> Result<Option<SystemPrompt>> {
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
        let prompt = self
            .storage
            .prompts()
            .find_by_id(prompt_id)
            .await?
            .ok_or_else(|| Error::PromptNotFound {
                id: prompt_id.to_string(),
            })?;

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
        let mut prompt = self
            .storage
            .prompts()
            .find_by_id(prompt_id)
            .await?
            .ok_or_else(|| Error::PromptNotFound {
                id: prompt_id.to_string(),
            })?;

        prompt.deactivate();
        self.storage.prompts().update(&prompt).await?;

        info!("Deactivated system prompt: {}", prompt.name);
        Ok(())
    }

    /// List all active prompts
    pub async fn list_active_prompts(&self) -> Result<Vec<SystemPrompt>> {
        self.storage
            .prompts()
            .list_active()
            .await
            .map_err(Error::Storage)
    }

    // === CACHING FUNCTIONALITY ===

    /// Render a prompt with caching support
    pub async fn render_prompt_cached(
        &self,
        prompt_id: Uuid,
        variables: HashMap<String, String>,
    ) -> Result<String> {
        // Create cache key
        let variables_json = serde_json::to_string(&variables)?;
        let cache_key = format!("{}:{}", prompt_id, self.hash_string(&variables_json));

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&cache_key) {
                if !self.is_cache_expired(entry) {
                    let rendered_content = entry.rendered_content.clone();
                    // Update access statistics
                    drop(cache);
                    self.update_cache_access(&cache_key).await;
                    return Ok(rendered_content);
                }
            }
        }

        // Cache miss - render and cache
        let content = self.render_prompt(prompt_id, variables.clone()).await?;
        self.cache_rendered_prompt(cache_key, prompt_id, variables_json, content.clone())
            .await?;

        Ok(content)
    }

    /// Clear the prompt cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Prompt cache cleared");
    }

    /// Clear expired cache entries
    pub async fn clear_expired_cache(&self) {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        cache.retain(|_, entry| entry.expires_at.map_or(true, |expires| expires > now));
        info!("Expired cache entries cleared");
    }

    // === A/B TESTING FUNCTIONALITY ===

    /// Create a new A/B test experiment
    pub async fn create_experiment(
        &self,
        name: String,
        description: String,
        prompt_a_id: Uuid,
        prompt_b_id: Uuid,
        allocation_percentage: f64,
        target_metric: MetricType,
        minimum_sample_size: u64,
        created_by: Uuid,
    ) -> Result<PromptExperiment> {
        if !(0.0..=100.0).contains(&allocation_percentage) {
            return Err(Error::Validation {
                message: "Allocation percentage must be between 0 and 100".to_string(),
            });
        }

        let experiment = PromptExperiment {
            id: Uuid::new_v4(),
            name,
            description,
            prompt_a_id,
            prompt_b_id,
            allocation_percentage,
            status: ExperimentStatus::Draft,
            start_date: Utc::now(),
            end_date: None,
            created_by,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            target_metric,
            minimum_sample_size,
            statistical_significance: None,
        };

        // TODO: Store experiment in database
        info!("Created A/B test experiment: {}", experiment.name);
        Ok(experiment)
    }

    /// Start an A/B test experiment
    pub async fn start_experiment(&self, experiment_id: Uuid) -> Result<()> {
        // TODO: Update experiment status to Running in database
        info!("Started A/B test experiment: {}", experiment_id);
        Ok(())
    }

    /// Stop an A/B test experiment
    pub async fn stop_experiment(&self, experiment_id: Uuid) -> Result<()> {
        // TODO: Update experiment status to Completed in database
        info!("Stopped A/B test experiment: {}", experiment_id);
        Ok(())
    }

    /// Get prompt based on A/B test allocation
    pub async fn get_prompt_for_experiment(
        &self,
        experiment_id: Uuid,
        agent_id: Option<Uuid>,
    ) -> Result<SystemPrompt> {
        // TODO: Implement actual A/B allocation logic
        // For now, return random allocation
        let use_prompt_b = rand::random::<f64>() < 0.5; // Simplified 50/50 allocation

        // TODO: Get experiment from database and use actual prompts
        // This is a placeholder implementation
        let prompt_id = if use_prompt_b {
            // Get prompt B from experiment
            Uuid::new_v4() // Placeholder
        } else {
            // Get prompt A from experiment
            Uuid::new_v4() // Placeholder
        };

        self.storage
            .prompts()
            .find_by_id(prompt_id)
            .await?
            .ok_or_else(|| Error::PromptNotFound {
                id: prompt_id.to_string(),
            })
    }

    /// Analyze A/B test results
    pub async fn analyze_experiment(&self, experiment_id: Uuid) -> Result<ExperimentResults> {
        // TODO: Implement statistical analysis of A/B test results
        // This would include:
        // - Calculating statistical significance
        // - Confidence intervals
        // - Recommendations based on results

        // Placeholder implementation
        let prompt_a_metrics = PromptMetrics {
            id: Uuid::new_v4(),
            prompt_id: Uuid::new_v4(),
            agent_id: None,
            usage_count: 100,
            success_rate: 0.85,
            average_response_time_ms: 150.0,
            quality_score: Some(8.5),
            user_feedback_score: Some(8.0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            period_start: Utc::now(),
            period_end: Utc::now(),
        };

        let prompt_b_metrics = PromptMetrics {
            id: Uuid::new_v4(),
            prompt_id: Uuid::new_v4(),
            agent_id: None,
            usage_count: 95,
            success_rate: 0.87,
            average_response_time_ms: 145.0,
            quality_score: Some(8.7),
            user_feedback_score: Some(8.2),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            period_start: Utc::now(),
            period_end: Utc::now(),
        };

        Ok(ExperimentResults {
            experiment_id,
            prompt_a_metrics,
            prompt_b_metrics,
            statistical_significance: 0.95,
            confidence_interval: (0.82, 0.92),
            recommendation: TestRecommendation::AdoptPromptB,
            analyzed_at: Utc::now(),
        })
    }

    // === METRICS AND FEEDBACK ===

    /// Record prompt usage metrics
    pub async fn record_usage_metrics(
        &self,
        prompt_id: Uuid,
        agent_id: Option<Uuid>,
        response_time_ms: f64,
        success: bool,
    ) -> Result<()> {
        // TODO: Update metrics in database
        info!(
            "Recorded usage metrics for prompt {}: success={}, response_time={}ms",
            prompt_id, success, response_time_ms
        );
        Ok(())
    }

    /// Submit feedback for a prompt
    pub async fn submit_feedback(
        &self,
        prompt_id: Uuid,
        agent_id: Option<Uuid>,
        task_id: Option<Uuid>,
        feedback_type: FeedbackType,
        score: f64,
        comments: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<PromptFeedback> {
        if !(0.0..=10.0).contains(&score) {
            return Err(Error::Validation {
                message: "Feedback score must be between 0.0 and 10.0".to_string(),
            });
        }

        let feedback = PromptFeedback {
            id: Uuid::new_v4(),
            prompt_id,
            agent_id,
            task_id,
            feedback_type,
            score,
            comments,
            metadata,
            created_at: Utc::now(),
        };

        // TODO: Store feedback in database
        info!(
            "Submitted feedback for prompt {}: score={}",
            prompt_id, score
        );
        Ok(feedback)
    }

    /// Get aggregated metrics for a prompt
    pub async fn get_prompt_metrics(
        &self,
        prompt_id: Uuid,
        period_start: Option<DateTime<Utc>>,
        period_end: Option<DateTime<Utc>>,
    ) -> Result<PromptMetrics> {
        // TODO: Calculate aggregated metrics from database
        // This would include:
        // - Usage count
        // - Success rate
        // - Average response time
        // - Quality scores
        // - User feedback scores

        // Placeholder implementation
        Ok(PromptMetrics {
            id: Uuid::new_v4(),
            prompt_id,
            agent_id: None,
            usage_count: 150,
            success_rate: 0.88,
            average_response_time_ms: 142.5,
            quality_score: Some(8.6),
            user_feedback_score: Some(8.1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            period_start: period_start.unwrap_or_else(Utc::now),
            period_end: period_end.unwrap_or_else(Utc::now),
        })
    }

    // === QUALITY ASSURANCE ===

    /// Validate a prompt against quality standards
    pub async fn validate_prompt_quality(&self, prompt: &SystemPrompt) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // Check template length
        if prompt.template.len() < 50 {
            issues.push("Prompt template is too short (minimum 50 characters)".to_string());
        }
        if prompt.template.len() > 10000 {
            issues.push("Prompt template is too long (maximum 10000 characters)".to_string());
        }

        // Check for required variables
        let template = &prompt.template;
        for variable in &prompt.variables {
            if variable.required {
                let placeholder = format!("{{{{{}}}}}", variable.name);
                if !template.contains(&placeholder) {
                    issues.push(format!(
                        "Required variable '{}' not found in template",
                        variable.name
                    ));
                }
            }
        }

        // Check for undefined variables in template
        let variable_names: std::collections::HashSet<_> =
            prompt.variables.iter().map(|v| &v.name).collect();

        // Simple regex to find {{variable}} patterns
        let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
        for captures in re.captures_iter(template) {
            if let Some(var_name) = captures.get(1) {
                if !variable_names.contains(&var_name.as_str().to_string()) {
                    issues.push(format!(
                        "Undefined variable '{}' found in template",
                        var_name.as_str()
                    ));
                }
            }
        }

        // Check for basic prompt structure
        if !template.to_lowercase().contains("you are")
            && !template.to_lowercase().contains("your role")
        {
            issues.push("Prompt should clearly define the agent's role".to_string());
        }

        Ok(issues)
    }

    /// Run automated quality checks on all active prompts
    pub async fn run_quality_audit(&self) -> Result<HashMap<Uuid, Vec<String>>> {
        let prompts = self.list_active_prompts().await?;
        let mut audit_results = HashMap::new();

        for prompt in prompts {
            let issues = self.validate_prompt_quality(&prompt).await?;
            if !issues.is_empty() {
                audit_results.insert(prompt.id, issues);
            }
        }

        info!(
            "Quality audit completed. Found issues in {} prompts",
            audit_results.len()
        );
        Ok(audit_results)
    }

    // === HOT-SWAPPING FUNCTIONALITY ===

    /// Hot-swap a prompt by creating a new version and activating it
    pub async fn hot_swap_prompt(
        &self,
        prompt_name: String,
        new_template: String,
        new_variables: Vec<PromptVariable>,
        created_by: Uuid,
    ) -> Result<SystemPrompt> {
        // Get current prompt
        let current_prompt = self
            .storage
            .prompts()
            .find_latest_by_name(&prompt_name)
            .await?
            .ok_or_else(|| Error::PromptNotFound {
                id: prompt_name.clone(),
            })?;

        // Update existing prompt with new content
        let mut updated_prompt = current_prompt;
        updated_prompt.template = new_template;
        updated_prompt.version += 1;
        updated_prompt.updated_at = chrono::Utc::now();

        // Clear existing variables and add new ones
        updated_prompt.variables.clear();
        for variable in new_variables {
            updated_prompt.add_variable(variable)?;
        }

        // Validate quality
        let issues = self.validate_prompt_quality(&updated_prompt).await?;
        if !issues.is_empty() {
            return Err(Error::Validation {
                message: format!("Quality validation failed: {}", issues.join(", ")),
            });
        }

        self.storage.prompts().update(&updated_prompt).await?;
        let new_prompt = updated_prompt;

        // Clear cache for this prompt
        self.clear_prompt_cache(&new_prompt.id).await;

        info!(
            "Hot-swapped prompt '{}' to version {}",
            new_prompt.name, new_prompt.version
        );
        Ok(new_prompt)
    }

    /// Clear cache entries for a specific prompt
    pub async fn clear_prompt_cache(&self, prompt_id: &Uuid) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, entry| entry.prompt_id != *prompt_id);
    }

    // === PRIVATE HELPER METHODS ===

    /// Hash a string for cache key generation
    fn hash_string(&self, s: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Check if a cache entry has expired
    fn is_cache_expired(&self, entry: &PromptCacheEntry) -> bool {
        if let Some(expires_at) = entry.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Update cache access statistics
    async fn update_cache_access(&self, cache_key: &str) {
        let mut cache = self.cache.write().await;
        if let Some(entry) = cache.get_mut(cache_key) {
            entry.access_count += 1;
            entry.last_accessed = Utc::now();
        }
    }

    /// Cache a rendered prompt
    async fn cache_rendered_prompt(
        &self,
        cache_key: String,
        prompt_id: Uuid,
        variables_hash: String,
        rendered_content: String,
    ) -> Result<()> {
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(self.cache_ttl).map_err(|e| Error::Validation {
                message: format!("Invalid cache TTL: {}", e),
            })?;

        let entry = PromptCacheEntry {
            key: cache_key.clone(),
            prompt_id,
            variables_hash,
            rendered_content,
            cached_at: now,
            access_count: 1,
            last_accessed: now,
            expires_at: Some(expires_at),
        };

        let mut cache = self.cache.write().await;
        cache.insert(cache_key, entry);
        Ok(())
    }

    /// Create default system prompts
    async fn create_default_prompts(&self) -> Result<()> {
        // Find or create a system agent to use as created_by
        let agents = self.storage.agents().list().await?;
        let system_id = if let Some(system_agent) = agents.iter().find(|a| a.name == "system") {
            system_agent.id
        } else {
            // Create a minimal system agent if it doesn't exist
            use chrono::Utc;
            use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};

            let system_agent = Agent::builder()
                .name("system")
                .agent_type(AgentType::Worker)
                .capability("system")
                .connection_metadata(ConnectionMetadata {
                    endpoint: "system://localhost".to_string(),
                    protocol_version: "1.0".to_string(),
                    session_id: Some("system".to_string()),
                    version: None,
                    transport: None,
                    capabilities: None,
                    session_type: None,
                    project_id: None,
                    coordination_scope: None,
                    specialization: None,
                    coordinator_managed: None,
                    workspace_isolation: None,
                })
                .build()
                .map_err(|e| Error::Validation {
                    message: format!("Failed to create system agent: {}", e),
                })?;

            let agent_id = system_agent.id;
            self.storage.agents().create(&system_agent).await?;
            agent_id
        };

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

        // Cross-project coordinator prompt
        let cross_project_coordinator_prompt = SystemPrompt::new(
            "Cross-Project Coordinator".to_string(),
            "Specialized prompt for cross-project coordination scenarios".to_string(),
            templates::CROSS_PROJECT_COORDINATOR_TEMPLATE.to_string(),
            PromptType::CrossProjectCoordinator,
            system_id,
        );

        // Conflict resolver prompt
        let conflict_resolver_prompt = SystemPrompt::new(
            "Conflict Resolver".to_string(),
            "Specialized prompt for conflict detection and resolution".to_string(),
            templates::CONFLICT_RESOLVER_TEMPLATE.to_string(),
            PromptType::ConflictResolver,
            system_id,
        );

        // Escalation manager prompt
        let escalation_manager_prompt = SystemPrompt::new(
            "Escalation Manager".to_string(),
            "Specialized prompt for managing escalation workflows".to_string(),
            templates::ESCALATION_MANAGER_TEMPLATE.to_string(),
            PromptType::EscalationManager,
            system_id,
        );

        // Add variables to prompts
        let mut coordinator_with_vars = coordinator_prompt?;
        coordinator_with_vars.add_variable(
            PromptVariable::new(
                "agent_name".to_string(),
                "Name of the agent".to_string(),
                VariableType::String,
                true,
            )
            .unwrap()
            .with_default_value("Coordinator".to_string()),
        )?;
        coordinator_with_vars.add_variable(
            PromptVariable::new(
                "team_size".to_string(),
                "Number of agents in the team".to_string(),
                VariableType::Number,
                false,
            )
            .unwrap()
            .with_default_value("1".to_string()),
        )?;

        let mut worker_with_vars = worker_prompt?;
        worker_with_vars.add_variable(
            PromptVariable::new(
                "agent_name".to_string(),
                "Name of the agent".to_string(),
                VariableType::String,
                true,
            )
            .unwrap()
            .with_default_value("Worker".to_string()),
        )?;
        worker_with_vars.add_variable(
            PromptVariable::new(
                "specialization".to_string(),
                "Agent's area of specialization".to_string(),
                VariableType::String,
                false,
            )
            .unwrap()
            .with_default_value("General".to_string()),
        )?;

        let universal_prompt = universal_prompt?;

        // Add variables to coordination prompts
        let mut cross_project_with_vars = cross_project_coordinator_prompt?;
        cross_project_with_vars.add_variable(
            PromptVariable::new(
                "agent_name".to_string(),
                "Name of the cross-project coordinator agent".to_string(),
                VariableType::String,
                true,
            )
            .unwrap()
            .with_default_value("Cross-Project Coordinator".to_string()),
        )?;

        let mut conflict_resolver_with_vars = conflict_resolver_prompt?;
        conflict_resolver_with_vars.add_variable(
            PromptVariable::new(
                "agent_name".to_string(),
                "Name of the conflict resolver agent".to_string(),
                VariableType::String,
                true,
            )
            .unwrap()
            .with_default_value("Conflict Resolver".to_string()),
        )?;

        let mut escalation_manager_with_vars = escalation_manager_prompt?;
        escalation_manager_with_vars.add_variable(
            PromptVariable::new(
                "agent_name".to_string(),
                "Name of the escalation manager agent".to_string(),
                VariableType::String,
                true,
            )
            .unwrap()
            .with_default_value("Escalation Manager".to_string()),
        )?;

        // Create prompts in storage
        self.storage
            .prompts()
            .create(&coordinator_with_vars)
            .await?;
        self.storage.prompts().create(&worker_with_vars).await?;
        self.storage.prompts().create(&universal_prompt).await?;
        self.storage
            .prompts()
            .create(&cross_project_with_vars)
            .await?;
        self.storage
            .prompts()
            .create(&conflict_resolver_with_vars)
            .await?;
        self.storage
            .prompts()
            .create(&escalation_manager_with_vars)
            .await?;

        info!("Created default system prompts including coordination specialists");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vibe_ensemble_core::agent::AgentType;
    use vibe_ensemble_core::prompt::{PromptType, PromptVariable, VariableType};
    use vibe_ensemble_storage::{manager::DatabaseConfig, StorageManager};

    async fn create_test_manager() -> PromptManager {
        use chrono::Utc;
        use vibe_ensemble_core::agent::{Agent, AgentStatus, AgentType, ConnectionMetadata};

        let config = DatabaseConfig {
            url: ":memory:".to_string(),
            max_connections: Some(1),
            migrate_on_startup: true,
            performance_config: None,
        };
        let storage = Arc::new(StorageManager::new(&config).await.unwrap());

        // Create a system agent that can be referenced as created_by
        let system_agent = Agent::builder()
            .name("system")
            .agent_type(AgentType::Worker)
            .capability("system")
            .connection_metadata(ConnectionMetadata {
                endpoint: "system://localhost".to_string(),
                protocol_version: "1.0".to_string(),
                session_id: Some("system".to_string()),
                version: None,
                transport: None,
                capabilities: None,
                session_type: None,
                project_id: None,
                coordination_scope: None,
                specialization: None,
                coordinator_managed: None,
                workspace_isolation: None,
            })
            .build()
            .unwrap();

        // Store the system agent
        storage.agents().create(&system_agent).await.unwrap();

        PromptManager::new(storage)
    }

    #[tokio::test]
    async fn test_initialization_creates_coordination_prompts() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        // Should have all 6 default prompts including coordination specialists
        assert_eq!(prompts.len(), 6);

        let prompt_types: Vec<_> = prompts.iter().map(|p| &p.prompt_type).collect();
        assert!(prompt_types.contains(&&PromptType::Coordinator));
        assert!(prompt_types.contains(&&PromptType::Worker));
        assert!(prompt_types.contains(&&PromptType::Universal));
        assert!(prompt_types.contains(&&PromptType::CrossProjectCoordinator));
        assert!(prompt_types.contains(&&PromptType::ConflictResolver));
        assert!(prompt_types.contains(&&PromptType::EscalationManager));
    }

    #[tokio::test]
    async fn test_coordination_prompts_have_required_content() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        // Test cross-project coordinator prompt
        let cross_project_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::CrossProjectCoordinator)
            .unwrap();

        assert!(cross_project_prompt
            .template
            .contains("vibe/cross-project/scan"));
        assert!(cross_project_prompt
            .template
            .contains("vibe/dependency/analyze"));
        assert!(cross_project_prompt
            .template
            .contains("vibe/project/coordinate"));
        assert!(cross_project_prompt
            .template
            .contains("bridge, not a controller"));

        // Test conflict resolver prompt
        let conflict_resolver_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::ConflictResolver)
            .unwrap();

        assert!(conflict_resolver_prompt
            .template
            .contains("vibe/conflict/detect"));
        assert!(conflict_resolver_prompt
            .template
            .contains("vibe/pattern/suggest"));
        assert!(conflict_resolver_prompt
            .template
            .contains("Graduated Response Protocol"));
        assert!(conflict_resolver_prompt
            .template
            .contains("resolution, not judgment"));

        // Test escalation manager prompt
        let escalation_manager_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::EscalationManager)
            .unwrap();

        assert!(escalation_manager_prompt
            .template
            .contains("Technical Escalations"));
        assert!(escalation_manager_prompt
            .template
            .contains("Process Escalations"));
        assert!(escalation_manager_prompt
            .template
            .contains("Business Escalations"));
        assert!(escalation_manager_prompt
            .template
            .contains("Information Package"));
    }

    #[tokio::test]
    async fn test_enhanced_coordinator_prompt_includes_coordination_intelligence() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let coordinator_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::Coordinator)
            .unwrap();

        // Check for coordination intelligence sections
        assert!(coordinator_prompt
            .template
            .contains("Coordination Intelligence"));
        assert!(coordinator_prompt.template.contains("Dependency Detection"));
        assert!(coordinator_prompt
            .template
            .contains("Conflict Resolution Protocols"));
        assert!(coordinator_prompt
            .template
            .contains("Escalation Decision Tree"));
        assert!(coordinator_prompt
            .template
            .contains("Communication Protocols"));
        assert!(coordinator_prompt.template.contains("Automation Triggers"));

        // Check for MCP tool integration
        assert!(coordinator_prompt
            .template
            .contains("vibe/dependency/analyze"));
        assert!(coordinator_prompt.template.contains("vibe/conflict/detect"));
        assert!(coordinator_prompt.template.contains("vibe/pattern/suggest"));
        assert!(coordinator_prompt
            .template
            .contains("vibe/coordination/mediate"));
        assert!(coordinator_prompt.template.contains("vibe/system/escalate"));
    }

    #[tokio::test]
    async fn test_enhanced_worker_prompt_includes_dependency_detection() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let worker_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::Worker)
            .unwrap();

        // Check for dependency detection sections
        assert!(worker_prompt
            .template
            .contains("Dependency Detection & Management"));
        assert!(worker_prompt.template.contains("Before Starting Work"));
        assert!(worker_prompt.template.contains("During Work Execution"));
        assert!(worker_prompt
            .template
            .contains("Intelligent Escalation Protocol"));
        assert!(worker_prompt
            .template
            .contains("Cross-Project Coordination"));

        // Check for escalation triggers
        assert!(worker_prompt
            .template
            .contains("Escalation Triggers (Immediate)"));
        assert!(worker_prompt
            .template
            .contains("Blocked by external dependencies for >2 hours"));
        assert!(worker_prompt
            .template
            .contains("Detecting conflicting changes"));

        // Check for MCP tool integration
        assert!(worker_prompt.template.contains("vibe/dependency/analyze"));
        assert!(worker_prompt.template.contains("vibe/cross-project/scan"));
        assert!(worker_prompt.template.contains("vibe/agent/message"));
        assert!(worker_prompt
            .template
            .contains("vibe/coordination/escalate"));
    }

    #[tokio::test]
    async fn test_coordination_prompt_variables() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        for prompt in &prompts {
            match prompt.prompt_type {
                PromptType::CrossProjectCoordinator
                | PromptType::ConflictResolver
                | PromptType::EscalationManager => {
                    // Should have agent_name variable
                    let agent_name_var = prompt.get_variable("agent_name");
                    assert!(agent_name_var.is_some());
                    assert!(agent_name_var.unwrap().required);
                    assert!(agent_name_var.unwrap().default_value.is_some());
                }
                _ => {} // Other prompts tested elsewhere
            }
        }
    }

    #[tokio::test]
    async fn test_coordination_prompt_rendering() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let cross_project_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::CrossProjectCoordinator)
            .unwrap();

        let mut variables = HashMap::new();
        variables.insert("agent_name".to_string(), "TestCoordinator".to_string());

        let rendered = manager
            .render_prompt(cross_project_prompt.id, variables)
            .await
            .unwrap();

        assert!(rendered.contains("TestCoordinator"));
        assert!(rendered.contains("Cross-Project Coordination Specialist"));
    }

    #[tokio::test]
    async fn test_coordination_prompt_caching() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let conflict_resolver_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::ConflictResolver)
            .unwrap();

        let mut variables = HashMap::new();
        variables.insert("agent_name".to_string(), "ConflictAgent".to_string());

        // First render - should cache
        let rendered1 = manager
            .render_prompt_cached(conflict_resolver_prompt.id, variables.clone())
            .await
            .unwrap();

        // Second render - should use cache
        let rendered2 = manager
            .render_prompt_cached(conflict_resolver_prompt.id, variables)
            .await
            .unwrap();

        assert_eq!(rendered1, rendered2);
        assert!(rendered1.contains("ConflictAgent"));
    }

    #[tokio::test]
    async fn test_coordination_prompt_quality_validation() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        for prompt in &prompts {
            match prompt.prompt_type {
                PromptType::CrossProjectCoordinator
                | PromptType::ConflictResolver
                | PromptType::EscalationManager => {
                    let validation_issues = manager.validate_prompt_quality(prompt).await.unwrap();
                    assert!(
                        validation_issues.is_empty(),
                        "Coordination prompt '{}' has validation issues: {:?}",
                        prompt.name,
                        validation_issues
                    );
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_prompt_suitability_for_coordination_specialists() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        // All coordination specialist prompts should be suitable for coordinators
        for prompt in &prompts {
            match prompt.prompt_type {
                PromptType::CrossProjectCoordinator
                | PromptType::ConflictResolver
                | PromptType::EscalationManager => {
                    assert!(prompt.is_suitable_for(&AgentType::Coordinator));
                    assert!(!prompt.is_suitable_for(&AgentType::Worker));
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_coordination_prompt_content_completeness() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();

        // Test that enhanced coordinator includes all required coordination elements
        let coordinator_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::Coordinator)
            .unwrap();

        let required_elements = [
            "Cross-project dependency orchestration",
            "Intelligent conflict detection and resolution",
            "Automated escalation management",
            "negotiate → isolate → escalate",
            "Agent Level",
            "Team Level",
            "System Level",
            "Human Level",
            "acknowledge → update → complete",
            "Automatic Actions",
            "Proactive Monitoring",
        ];

        for element in &required_elements {
            assert!(
                coordinator_prompt.template.contains(element),
                "Coordinator prompt missing required element: {}",
                element
            );
        }

        // Test that enhanced worker includes all required coordination elements
        let worker_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::Worker)
            .unwrap();

        let worker_required_elements = [
            "Proactively detect and communicate dependencies",
            "Coordinate directly with other workers when appropriate",
            "Self-Resolution (Preferred)",
            "Escalation Triggers (Immediate)",
            "Communication Etiquette",
            "Lead with context: what you're doing and why",
            "use your tools proactively to prevent problems",
        ];

        for element in &worker_required_elements {
            assert!(
                worker_prompt.template.contains(element),
                "Worker prompt missing required element: {}",
                element
            );
        }
    }

    #[tokio::test]
    async fn test_hot_swap_coordination_prompts() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        // Get the system agent's ID
        let agents = manager.storage.agents().list().await.unwrap();
        let system_id = agents.iter().find(|a| a.name == "system").unwrap().id;

        let new_template = r#"
        You are {{agent_name}}, an Enhanced Cross-Project Coordinator.
        This is a hot-swapped version with improved capabilities.
        
        ## Enhanced Features
        - Advanced dependency mapping with AI prediction
        - Real-time conflict prevention algorithms  
        - Automated cross-team communication protocols
        "#
        .to_string();

        let variables = vec![PromptVariable::new(
            "agent_name".to_string(),
            "Name of the enhanced coordinator".to_string(),
            VariableType::String,
            true,
        )
        .unwrap()
        .with_default_value("Enhanced Coordinator".to_string())];

        let swapped_prompt = manager
            .hot_swap_prompt(
                "Cross-Project Coordinator".to_string(),
                new_template,
                variables,
                system_id,
            )
            .await
            .unwrap();

        assert!(swapped_prompt
            .template
            .contains("Enhanced Cross-Project Coordinator"));
        assert!(swapped_prompt
            .template
            .contains("Advanced dependency mapping"));
        assert_eq!(swapped_prompt.version, 2);
        assert!(swapped_prompt.is_active);
    }

    #[tokio::test]
    async fn test_coordination_metrics_and_feedback() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let escalation_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::EscalationManager)
            .unwrap();

        // Test recording usage metrics
        manager
            .record_usage_metrics(escalation_prompt.id, None, 120.5, true)
            .await
            .unwrap();

        // Test submitting feedback
        let mut feedback_metadata = HashMap::new();
        feedback_metadata.insert("scenario".to_string(), "cross_project_conflict".to_string());
        feedback_metadata.insert("complexity".to_string(), "high".to_string());

        let feedback = manager
            .submit_feedback(
                escalation_prompt.id,
                None,
                None,
                FeedbackType::QualityAssurance,
                8.5,
                Some("Effective escalation handling with clear decision tree".to_string()),
                feedback_metadata,
            )
            .await
            .unwrap();

        assert_eq!(feedback.prompt_id, escalation_prompt.id);
        assert_eq!(feedback.score, 8.5);
        assert_eq!(feedback.feedback_type, FeedbackType::QualityAssurance);
        assert!(feedback.comments.is_some());
        assert_eq!(feedback.metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_coordination_ab_testing() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let coordinator_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::Coordinator)
            .unwrap();
        let cross_project_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::CrossProjectCoordinator)
            .unwrap();

        let system_id = Uuid::new_v4();

        // Create A/B test experiment between regular coordinator and cross-project specialist
        let experiment = manager
            .create_experiment(
                "Coordination Effectiveness Test".to_string(),
                "Testing regular coordinator vs specialized cross-project coordinator".to_string(),
                coordinator_prompt.id,
                cross_project_prompt.id,
                30.0,
                MetricType::QualityScore,
                50,
                system_id,
            )
            .await
            .unwrap();

        assert_eq!(experiment.name, "Coordination Effectiveness Test");
        assert_eq!(experiment.prompt_a_id, coordinator_prompt.id);
        assert_eq!(experiment.prompt_b_id, cross_project_prompt.id);
        assert_eq!(experiment.allocation_percentage, 30.0);
        assert_eq!(experiment.target_metric, MetricType::QualityScore);
        assert_eq!(experiment.minimum_sample_size, 50);

        // Test starting experiment
        manager.start_experiment(experiment.id).await.unwrap();

        // Test analyzing results
        let results = manager.analyze_experiment(experiment.id).await.unwrap();
        assert_eq!(results.experiment_id, experiment.id);
        assert!(results.statistical_significance > 0.0);
    }

    #[tokio::test]
    async fn test_cache_management_with_coordination_prompts() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let prompts = manager.list_active_prompts().await.unwrap();
        let conflict_prompt = prompts
            .iter()
            .find(|p| p.prompt_type == PromptType::ConflictResolver)
            .unwrap();

        let mut variables = HashMap::new();
        variables.insert("agent_name".to_string(), "TestResolver".to_string());

        // Render with caching
        manager
            .render_prompt_cached(conflict_prompt.id, variables)
            .await
            .unwrap();

        // Clear specific prompt cache
        manager.clear_prompt_cache(&conflict_prompt.id).await;

        // Clear expired cache
        manager.clear_expired_cache().await;

        // Clear all cache
        manager.clear_cache().await;
    }

    #[tokio::test]
    async fn test_quality_audit_includes_coordination_prompts() {
        let manager = create_test_manager().await;
        manager.initialize().await.unwrap();

        let audit_results = manager.run_quality_audit().await.unwrap();

        // All coordination prompts should pass quality audit
        assert!(
            audit_results.is_empty(),
            "Quality audit found issues in coordination prompts: {:?}",
            audit_results
        );
    }
}
