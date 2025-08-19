//! Prompt management functionality

use crate::{renderer::PromptRenderer, templates, Error, Result};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
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
                    // Update access statistics
                    drop(cache);
                    self.update_cache_access(&cache_key).await;
                    return Ok(entry.rendered_content.clone());
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
        if allocation_percentage < 0.0 || allocation_percentage > 100.0 {
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
        if score < 0.0 || score > 10.0 {
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
            period_start: period_start.unwrap_or_else(|| Utc::now()),
            period_end: period_end.unwrap_or_else(|| Utc::now()),
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

        // Create new version
        let mut new_prompt = SystemPrompt::new(
            current_prompt.name.clone(),
            current_prompt.description.clone(),
            new_template,
            current_prompt.prompt_type.clone(),
            created_by,
        )?;

        // Set version number
        new_prompt.version = current_prompt.version + 1;

        // Add variables
        for variable in new_variables {
            new_prompt.add_variable(variable)?;
        }

        // Validate quality
        let issues = self.validate_prompt_quality(&new_prompt).await?;
        if !issues.is_empty() {
            return Err(Error::Validation {
                message: format!("Quality validation failed: {}", issues.join(", ")),
            });
        }

        // Deactivate old version
        let mut old_prompt = current_prompt;
        old_prompt.deactivate();
        self.storage.prompts().update(&old_prompt).await?;

        // Create and activate new version
        self.storage.prompts().create(&new_prompt).await?;

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

        // Create prompts in storage
        self.storage
            .prompts()
            .create(&coordinator_with_vars)
            .await?;
        self.storage.prompts().create(&worker_with_vars).await?;
        self.storage.prompts().create(&universal_prompt).await?;

        info!("Created default system prompts");
        Ok(())
    }
}
