//! Configuration domain model and related types
//!
//! This module provides the core configuration model for managing
//! coordinator settings, behavioral parameters, and integration specifications
//! in the Vibe Ensemble system.
//!
//! # Examples
//!
//! Creating a new configuration:
//!
//! ```rust
//! use vibe_ensemble_core::config::*;
//! use uuid::Uuid;
//!
//! let config = Configuration::builder()
//!     .name("production-coordinator")
//!     .coordination_settings(CoordinationSettings::builder()
//!         .max_concurrent_tasks(10)
//!         .task_timeout_seconds(3600)
//!         .heartbeat_interval_seconds(30)
//!         .build()
//!         .unwrap())
//!     .behavioral_parameter("max_retries", "3")
//!     .behavioral_parameter("log_level", "info")
//!     .integration_spec("github", IntegrationSpec::new(
//!         "GitHub Integration".to_string(),
//!         "https://api.github.com".to_string(),
//!         vec![("api_key".to_string(), "secret".to_string())],
//!     ).unwrap())
//!     .build()
//!     .unwrap();
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use crate::{Error, Result};

/// Represents a configuration for the Vibe Ensemble system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Configuration {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub coordination_settings: CoordinationSettings,
    pub behavioral_parameters: HashMap<String, String>,
    pub integration_specs: HashMap<String, IntegrationSpec>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub is_active: bool,
}

/// Coordinator settings for task and agent management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationSettings {
    pub max_concurrent_tasks: u32,
    pub task_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
    pub max_agents: u32,
    pub auto_scaling_enabled: bool,
    pub load_balancing_strategy: LoadBalancingStrategy,
    pub failure_handling: FailureHandlingStrategy,
}

/// Load balancing strategies for task distribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastLoaded,
    CapabilityBased,
    Random,
}

/// Failure handling strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureHandlingStrategy {
    Retry { max_attempts: u32, backoff_seconds: u64 },
    Failover { backup_agents: Vec<Uuid> },
    Abort,
    Escalate { escalation_agent_id: Uuid },
}

/// Integration specification for external services
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntegrationSpec {
    pub name: String,
    pub endpoint: String,
    pub credentials: Vec<(String, String)>, // key-value pairs for credentials
    pub timeout_seconds: u64,
    pub retry_policy: RetryPolicy,
    pub enabled: bool,
}

/// Retry policy for external integrations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay_seconds: u64,
    pub backoff_multiplier: f64,
    pub max_delay_seconds: u64,
}

impl Configuration {
    /// Create a new configuration with validation
    pub fn new(
        name: String,
        coordination_settings: CoordinationSettings,
    ) -> Result<Self> {
        Self::validate_name(&name)?;
        
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            coordination_settings,
            behavioral_parameters: HashMap::new(),
            integration_specs: HashMap::new(),
            created_at: now,
            updated_at: now,
            version: 1,
            is_active: true,
        })
    }

    /// Create a builder for constructing a Configuration
    pub fn builder() -> ConfigurationBuilder {
        ConfigurationBuilder::new()
    }

    /// Validate configuration name
    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Configuration name cannot be empty".to_string(),
            });
        }
        if name.len() > 100 {
            return Err(Error::Validation {
                message: "Configuration name cannot exceed 100 characters".to_string(),
            });
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(Error::Validation {
                message: "Configuration name can only contain alphanumeric characters, hyphens, and underscores".to_string(),
            });
        }
        Ok(())
    }

    /// Add a behavioral parameter
    pub fn add_behavioral_parameter(&mut self, key: String, value: String) -> Result<()> {
        if key.trim().is_empty() {
            return Err(Error::Validation {
                message: "Behavioral parameter key cannot be empty".to_string(),
            });
        }
        if value.trim().is_empty() {
            return Err(Error::Validation {
                message: "Behavioral parameter value cannot be empty".to_string(),
            });
        }
        self.behavioral_parameters.insert(key, value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a behavioral parameter
    pub fn remove_behavioral_parameter(&mut self, key: &str) {
        if self.behavioral_parameters.remove(key).is_some() {
            self.updated_at = Utc::now();
        }
    }

    /// Get a behavioral parameter value
    pub fn get_behavioral_parameter(&self, key: &str) -> Option<&String> {
        self.behavioral_parameters.get(key)
    }

    /// Add an integration specification
    pub fn add_integration_spec(&mut self, key: String, spec: IntegrationSpec) -> Result<()> {
        if key.trim().is_empty() {
            return Err(Error::Validation {
                message: "Integration spec key cannot be empty".to_string(),
            });
        }
        self.integration_specs.insert(key, spec);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove an integration specification
    pub fn remove_integration_spec(&mut self, key: &str) {
        if self.integration_specs.remove(key).is_some() {
            self.updated_at = Utc::now();
        }
    }

    /// Get an integration specification
    pub fn get_integration_spec(&self, key: &str) -> Option<&IntegrationSpec> {
        self.integration_specs.get(key)
    }

    /// Update coordination settings
    pub fn update_coordination_settings(&mut self, settings: CoordinationSettings) {
        self.coordination_settings = settings;
        self.updated_at = Utc::now();
        self.version += 1;
    }

    /// Set the description
    pub fn set_description(&mut self, description: Option<String>) {
        if let Some(desc) = &description {
            if desc.len() > 1000 {
                return; // Silently ignore overly long descriptions
            }
        }
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// Activate the configuration
    pub fn activate(&mut self) {
        if !self.is_active {
            self.is_active = true;
            self.updated_at = Utc::now();
        }
    }

    /// Deactivate the configuration
    pub fn deactivate(&mut self) {
        if self.is_active {
            self.is_active = false;
            self.updated_at = Utc::now();
        }
    }

    /// Get the age of the configuration in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }

    /// Get the time since last update in seconds
    pub fn time_since_update_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.updated_at).num_seconds()
    }
}

impl CoordinationSettings {
    /// Create a new coordination settings builder
    pub fn builder() -> CoordinationSettingsBuilder {
        CoordinationSettingsBuilder::new()
    }

    /// Create default coordination settings
    pub fn default_settings() -> Self {
        Self {
            max_concurrent_tasks: 5,
            task_timeout_seconds: 1800, // 30 minutes
            heartbeat_interval_seconds: 60, // 1 minute
            max_agents: 10,
            auto_scaling_enabled: false,
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            failure_handling: FailureHandlingStrategy::Retry {
                max_attempts: 3,
                backoff_seconds: 5,
            },
        }
    }

    /// Validate the settings
    pub fn validate(&self) -> Result<()> {
        if self.max_concurrent_tasks == 0 {
            return Err(Error::Validation {
                message: "Max concurrent tasks must be greater than 0".to_string(),
            });
        }
        if self.task_timeout_seconds == 0 {
            return Err(Error::Validation {
                message: "Task timeout must be greater than 0".to_string(),
            });
        }
        if self.heartbeat_interval_seconds == 0 {
            return Err(Error::Validation {
                message: "Heartbeat interval must be greater than 0".to_string(),
            });
        }
        if self.max_agents == 0 {
            return Err(Error::Validation {
                message: "Max agents must be greater than 0".to_string(),
            });
        }
        Ok(())
    }
}

impl IntegrationSpec {
    /// Create a new integration spec with validation
    pub fn new(
        name: String,
        endpoint: String,
        credentials: Vec<(String, String)>,
    ) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                message: "Integration name cannot be empty".to_string(),
            });
        }
        if endpoint.trim().is_empty() {
            return Err(Error::Validation {
                message: "Integration endpoint cannot be empty".to_string(),
            });
        }
        
        Ok(Self {
            name,
            endpoint,
            credentials,
            timeout_seconds: 30,
            retry_policy: RetryPolicy::default(),
            enabled: true,
        })
    }

    /// Enable the integration
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the integration
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Set timeout
    pub fn set_timeout(&mut self, timeout_seconds: u64) -> Result<()> {
        if timeout_seconds == 0 {
            return Err(Error::Validation {
                message: "Timeout must be greater than 0".to_string(),
            });
        }
        self.timeout_seconds = timeout_seconds;
        Ok(())
    }

    /// Update retry policy
    pub fn set_retry_policy(&mut self, policy: RetryPolicy) {
        self.retry_policy = policy;
    }
}

impl RetryPolicy {
    /// Create a new retry policy
    pub fn new(
        max_attempts: u32,
        initial_delay_seconds: u64,
        backoff_multiplier: f64,
        max_delay_seconds: u64,
    ) -> Result<Self> {
        if max_attempts == 0 {
            return Err(Error::Validation {
                message: "Max attempts must be greater than 0".to_string(),
            });
        }
        if backoff_multiplier <= 0.0 {
            return Err(Error::Validation {
                message: "Backoff multiplier must be greater than 0".to_string(),
            });
        }
        if max_delay_seconds < initial_delay_seconds {
            return Err(Error::Validation {
                message: "Max delay must be greater than or equal to initial delay".to_string(),
            });
        }
        
        Ok(Self {
            max_attempts,
            initial_delay_seconds,
            backoff_multiplier,
            max_delay_seconds,
        })
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_seconds: 1,
            backoff_multiplier: 2.0,
            max_delay_seconds: 60,
        }
    }
}

/// Builder for constructing Configuration instances with validation
#[derive(Debug, Clone)]
pub struct ConfigurationBuilder {
    name: Option<String>,
    description: Option<String>,
    coordination_settings: Option<CoordinationSettings>,
    behavioral_parameters: HashMap<String, String>,
    integration_specs: HashMap<String, IntegrationSpec>,
}

impl ConfigurationBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            coordination_settings: None,
            behavioral_parameters: HashMap::new(),
            integration_specs: HashMap::new(),
        }
    }

    /// Set the configuration name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the configuration description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the coordination settings
    pub fn coordination_settings(mut self, settings: CoordinationSettings) -> Self {
        self.coordination_settings = Some(settings);
        self
    }

    /// Add a behavioral parameter
    pub fn behavioral_parameter<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.behavioral_parameters.insert(key.into(), value.into());
        self
    }

    /// Add multiple behavioral parameters
    pub fn behavioral_parameters<I, K, V>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.behavioral_parameters.extend(
            params.into_iter().map(|(k, v)| (k.into(), v.into()))
        );
        self
    }

    /// Add an integration spec
    pub fn integration_spec<K: Into<String>>(mut self, key: K, spec: IntegrationSpec) -> Self {
        self.integration_specs.insert(key.into(), spec);
        self
    }

    /// Build the Configuration instance
    pub fn build(self) -> Result<Configuration> {
        let name = self.name.ok_or_else(|| Error::Validation {
            message: "Configuration name is required".to_string(),
        })?;
        let coordination_settings = self.coordination_settings.unwrap_or_else(CoordinationSettings::default_settings);
        
        coordination_settings.validate()?;
        
        let mut config = Configuration::new(name, coordination_settings)?;
        
        if let Some(description) = self.description {
            config.set_description(Some(description));
        }
        
        // Add behavioral parameters
        for (key, value) in self.behavioral_parameters {
            config.add_behavioral_parameter(key, value)?;
        }
        
        // Add integration specs
        for (key, spec) in self.integration_specs {
            config.add_integration_spec(key, spec)?;
        }
        
        Ok(config)
    }
}

impl Default for ConfigurationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing CoordinationSettings instances
#[derive(Debug, Clone)]
pub struct CoordinationSettingsBuilder {
    max_concurrent_tasks: u32,
    task_timeout_seconds: u64,
    heartbeat_interval_seconds: u64,
    max_agents: u32,
    auto_scaling_enabled: bool,
    load_balancing_strategy: LoadBalancingStrategy,
    failure_handling: FailureHandlingStrategy,
}

impl CoordinationSettingsBuilder {
    /// Create a new coordination settings builder
    pub fn new() -> Self {
        let defaults = CoordinationSettings::default_settings();
        Self {
            max_concurrent_tasks: defaults.max_concurrent_tasks,
            task_timeout_seconds: defaults.task_timeout_seconds,
            heartbeat_interval_seconds: defaults.heartbeat_interval_seconds,
            max_agents: defaults.max_agents,
            auto_scaling_enabled: defaults.auto_scaling_enabled,
            load_balancing_strategy: defaults.load_balancing_strategy,
            failure_handling: defaults.failure_handling,
        }
    }

    /// Set max concurrent tasks
    pub fn max_concurrent_tasks(mut self, max: u32) -> Self {
        self.max_concurrent_tasks = max;
        self
    }

    /// Set task timeout
    pub fn task_timeout_seconds(mut self, timeout: u64) -> Self {
        self.task_timeout_seconds = timeout;
        self
    }

    /// Set heartbeat interval
    pub fn heartbeat_interval_seconds(mut self, interval: u64) -> Self {
        self.heartbeat_interval_seconds = interval;
        self
    }

    /// Set max agents
    pub fn max_agents(mut self, max: u32) -> Self {
        self.max_agents = max;
        self
    }

    /// Enable auto scaling
    pub fn enable_auto_scaling(mut self) -> Self {
        self.auto_scaling_enabled = true;
        self
    }

    /// Disable auto scaling
    pub fn disable_auto_scaling(mut self) -> Self {
        self.auto_scaling_enabled = false;
        self
    }

    /// Set load balancing strategy
    pub fn load_balancing_strategy(mut self, strategy: LoadBalancingStrategy) -> Self {
        self.load_balancing_strategy = strategy;
        self
    }

    /// Set failure handling strategy
    pub fn failure_handling(mut self, strategy: FailureHandlingStrategy) -> Self {
        self.failure_handling = strategy;
        self
    }

    /// Build the CoordinationSettings instance
    pub fn build(self) -> Result<CoordinationSettings> {
        let settings = CoordinationSettings {
            max_concurrent_tasks: self.max_concurrent_tasks,
            task_timeout_seconds: self.task_timeout_seconds,
            heartbeat_interval_seconds: self.heartbeat_interval_seconds,
            max_agents: self.max_agents,
            auto_scaling_enabled: self.auto_scaling_enabled,
            load_balancing_strategy: self.load_balancing_strategy,
            failure_handling: self.failure_handling,
        };
        
        settings.validate()?;
        Ok(settings)
    }
}

impl Default for CoordinationSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configuration_creation_with_builder() {
        let coordination_settings = CoordinationSettings::builder()
            .max_concurrent_tasks(10)
            .task_timeout_seconds(3600)
            .heartbeat_interval_seconds(30)
            .max_agents(20)
            .enable_auto_scaling()
            .load_balancing_strategy(LoadBalancingStrategy::LeastLoaded)
            .build()
            .unwrap();

        let config = Configuration::builder()
            .name("test-config")
            .description("Test configuration for validation")
            .coordination_settings(coordination_settings)
            .behavioral_parameter("max_retries", "5")
            .behavioral_parameter("log_level", "debug")
            .build()
            .unwrap();

        assert_eq!(config.name, "test-config");
        assert!(config.description.is_some());
        assert_eq!(config.coordination_settings.max_concurrent_tasks, 10);
        assert_eq!(config.coordination_settings.auto_scaling_enabled, true);
        assert_eq!(config.behavioral_parameters.len(), 2);
        assert_eq!(config.get_behavioral_parameter("max_retries"), Some(&"5".to_string()));
        assert!(config.is_active);
        assert_eq!(config.version, 1);
    }

    #[test]
    fn test_configuration_name_validation() {
        // Empty name should fail
        let result = Configuration::builder()
            .name("")
            .build();
        assert!(result.is_err());

        // Invalid characters should fail
        let result = Configuration::builder()
            .name("test@config")
            .build();
        assert!(result.is_err());

        // Too long name should fail
        let long_name = "a".repeat(101);
        let result = Configuration::builder()
            .name(long_name)
            .build();
        assert!(result.is_err());

        // Valid name should succeed
        let result = Configuration::builder()
            .name("valid-config_123")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_coordination_settings_validation() {
        // Zero max concurrent tasks should fail
        let result = CoordinationSettings::builder()
            .max_concurrent_tasks(0)
            .build();
        assert!(result.is_err());

        // Zero task timeout should fail
        let result = CoordinationSettings::builder()
            .task_timeout_seconds(0)
            .build();
        assert!(result.is_err());

        // Zero heartbeat interval should fail
        let result = CoordinationSettings::builder()
            .heartbeat_interval_seconds(0)
            .build();
        assert!(result.is_err());

        // Zero max agents should fail
        let result = CoordinationSettings::builder()
            .max_agents(0)
            .build();
        assert!(result.is_err());

        // Valid settings should succeed
        let result = CoordinationSettings::builder()
            .max_concurrent_tasks(5)
            .task_timeout_seconds(1800)
            .heartbeat_interval_seconds(60)
            .max_agents(10)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_behavioral_parameters() {
        let mut config = Configuration::builder()
            .name("test-config")
            .build()
            .unwrap();

        // Add parameter
        config.add_behavioral_parameter("test_param".to_string(), "test_value".to_string()).unwrap();
        assert_eq!(config.get_behavioral_parameter("test_param"), Some(&"test_value".to_string()));

        // Remove parameter
        config.remove_behavioral_parameter("test_param");
        assert_eq!(config.get_behavioral_parameter("test_param"), None);

        // Empty key should fail
        let result = config.add_behavioral_parameter("".to_string(), "value".to_string());
        assert!(result.is_err());

        // Empty value should fail
        let result = config.add_behavioral_parameter("key".to_string(), "".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_specs() {
        let mut config = Configuration::builder()
            .name("test-config")
            .build()
            .unwrap();

        let spec = IntegrationSpec::new(
            "Test Integration".to_string(),
            "https://api.example.com".to_string(),
            vec![("api_key".to_string(), "secret".to_string())],
        ).unwrap();

        // Add integration spec
        config.add_integration_spec("test_integration".to_string(), spec.clone()).unwrap();
        assert!(config.get_integration_spec("test_integration").is_some());

        // Remove integration spec
        config.remove_integration_spec("test_integration");
        assert!(config.get_integration_spec("test_integration").is_none());

        // Empty key should fail
        let result = config.add_integration_spec("".to_string(), spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_spec_validation() {
        // Empty name should fail
        let result = IntegrationSpec::new(
            "".to_string(),
            "https://api.example.com".to_string(),
            vec![],
        );
        assert!(result.is_err());

        // Empty endpoint should fail
        let result = IntegrationSpec::new(
            "Test Integration".to_string(),
            "".to_string(),
            vec![],
        );
        assert!(result.is_err());

        // Valid spec should succeed
        let result = IntegrationSpec::new(
            "Test Integration".to_string(),
            "https://api.example.com".to_string(),
            vec![("key".to_string(), "value".to_string())],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_spec_operations() {
        let mut spec = IntegrationSpec::new(
            "Test Integration".to_string(),
            "https://api.example.com".to_string(),
            vec![],
        ).unwrap();

        assert!(spec.enabled);
        
        spec.disable();
        assert!(!spec.enabled);
        
        spec.enable();
        assert!(spec.enabled);

        // Set timeout
        spec.set_timeout(60).unwrap();
        assert_eq!(spec.timeout_seconds, 60);

        // Zero timeout should fail
        let result = spec.set_timeout(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_policy() {
        // Valid policy
        let policy = RetryPolicy::new(5, 2, 1.5, 30).unwrap();
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_delay_seconds, 2);
        assert_eq!(policy.backoff_multiplier, 1.5);
        assert_eq!(policy.max_delay_seconds, 30);

        // Zero max attempts should fail
        let result = RetryPolicy::new(0, 1, 2.0, 10);
        assert!(result.is_err());

        // Zero or negative backoff multiplier should fail
        let result = RetryPolicy::new(3, 1, 0.0, 10);
        assert!(result.is_err());

        // Max delay less than initial delay should fail
        let result = RetryPolicy::new(3, 10, 2.0, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_configuration_lifecycle() {
        let mut config = Configuration::builder()
            .name("test-config")
            .build()
            .unwrap();

        assert!(config.is_active);
        
        config.deactivate();
        assert!(!config.is_active);
        
        config.activate();
        assert!(config.is_active);

        // Test description setting
        config.set_description(Some("New description".to_string()));
        assert_eq!(config.description, Some("New description".to_string()));

        // Test age and time since update
        let age = config.age_seconds();
        let time_since_update = config.time_since_update_seconds();
        assert!(age >= 0);
        assert!(time_since_update >= 0);
    }

    #[test]
    fn test_coordination_settings_update() {
        let mut config = Configuration::builder()
            .name("test-config")
            .build()
            .unwrap();

        let initial_version = config.version;
        
        let new_settings = CoordinationSettings::builder()
            .max_concurrent_tasks(20)
            .build()
            .unwrap();

        config.update_coordination_settings(new_settings);
        assert_eq!(config.coordination_settings.max_concurrent_tasks, 20);
        assert_eq!(config.version, initial_version + 1);
    }

    #[test]
    fn test_default_coordination_settings() {
        let settings = CoordinationSettings::default_settings();
        assert_eq!(settings.max_concurrent_tasks, 5);
        assert_eq!(settings.task_timeout_seconds, 1800);
        assert_eq!(settings.heartbeat_interval_seconds, 60);
        assert_eq!(settings.max_agents, 10);
        assert!(!settings.auto_scaling_enabled);
        assert_eq!(settings.load_balancing_strategy, LoadBalancingStrategy::RoundRobin);
    }

    #[test]
    fn test_failure_handling_strategies() {
        let retry_strategy = FailureHandlingStrategy::Retry {
            max_attempts: 3,
            backoff_seconds: 5,
        };
        
        let failover_strategy = FailureHandlingStrategy::Failover {
            backup_agents: vec![Uuid::new_v4(), Uuid::new_v4()],
        };
        
        let escalate_strategy = FailureHandlingStrategy::Escalate {
            escalation_agent_id: Uuid::new_v4(),
        };

        // Test that different strategies can be used
        let config1 = Configuration::builder()
            .name("config1")
            .coordination_settings(CoordinationSettings::builder()
                .failure_handling(retry_strategy)
                .build()
                .unwrap())
            .build()
            .unwrap();

        let config2 = Configuration::builder()
            .name("config2")
            .coordination_settings(CoordinationSettings::builder()
                .failure_handling(failover_strategy)
                .build()
                .unwrap())
            .build()
            .unwrap();

        let config3 = Configuration::builder()
            .name("config3")
            .coordination_settings(CoordinationSettings::builder()
                .failure_handling(escalate_strategy)
                .build()
                .unwrap())
            .build()
            .unwrap();

        // Verify they all built successfully
        assert_eq!(config1.name, "config1");
        assert_eq!(config2.name, "config2");
        assert_eq!(config3.name, "config3");
    }
}