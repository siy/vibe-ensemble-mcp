//! Agent domain model and related types
//!
//! This module provides the core agent model for representing Claude Code agents
//! in the Vibe Ensemble system. Agents are the primary entities that coordinate
//! and execute tasks.
//!
//! # Examples
//!
//! Creating a new agent:
//!
//! ```rust
//! use vibe_ensemble_core::agent::*;
//!
//! let metadata = ConnectionMetadata::builder()
//!     .endpoint("http://localhost:8080")
//!     .protocol_version("1.0")
//!     .build()
//!     .unwrap();
//!
//! let agent = Agent::builder()
//!     .name("claude-worker-01")
//!     .agent_type(AgentType::Worker)
//!     .capability("code-generation")
//!     .capability("testing")
//!     .connection_metadata(metadata)
//!     .build()
//!     .unwrap();
//! ```

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a Claude Code agent in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub agent_type: AgentType,
    pub capabilities: Vec<String>,
    pub status: AgentStatus,
    pub connection_metadata: ConnectionMetadata,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub performance_metrics: AgentPerformanceMetrics,
    pub resource_allocation: ResourceAllocation,
    pub health_info: HealthInfo,
    pub system_prompt_assignment: Option<SystemPromptAssignment>,
}

/// Performance metrics for an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPerformanceMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub average_response_time_ms: f64,
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub last_task_completed_at: Option<DateTime<Utc>>,
}

/// Resource allocation information for an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceAllocation {
    pub max_concurrent_tasks: u32,
    pub current_task_count: u32,
    pub load_factor: f32,    // 0.0 to 1.0
    pub priority_level: u32, // Higher number = higher priority
}

/// Health monitoring information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthInfo {
    pub consecutive_failures: u32,
    pub last_health_check: DateTime<Utc>,
    pub recovery_attempts: u32,
    pub quarantine_until: Option<DateTime<Utc>>,
}

/// Type of agent in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
    Coordinator,
    Worker,
}

/// Current status of an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    /// Agent is connecting to the system
    Connecting,
    /// Agent is online and available for work
    Online,
    /// Agent is busy executing a task
    Busy,
    /// Agent is temporarily unavailable but connected
    Idle,
    /// Agent is in maintenance mode
    Maintenance,
    /// Agent is disconnecting gracefully
    Disconnecting,
    /// Agent is offline
    Offline,
    /// Agent encountered an error
    Error { message: String },
    /// Agent failed health checks
    Unhealthy { reason: String },
}

/// Connection metadata for an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectionMetadata {
    pub endpoint: String,
    pub protocol_version: String,
    pub session_id: Option<String>,
    pub version: Option<String>,
    pub transport: Option<String>,
    pub capabilities: Option<String>,
    pub session_type: Option<String>,
    pub project_context: Option<String>,
    pub coordination_scope: Option<String>,
    pub specialization: Option<String>,
    pub coordinator_managed: Option<bool>,
    pub workspace_isolation: Option<bool>,
}

/// System prompt assignment information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemPromptAssignment {
    pub prompt_id: Option<uuid::Uuid>,
    pub prompt_version: Option<String>,
    pub assigned_at: DateTime<Utc>,
    pub context_data: std::collections::HashMap<String, String>,
}

impl Default for SystemPromptAssignment {
    fn default() -> Self {
        Self {
            prompt_id: None,
            prompt_version: None,
            assigned_at: Utc::now(),
            context_data: std::collections::HashMap::new(),
        }
    }
}

impl Agent {
    /// Create a new agent instance with validation
    pub fn new(
        name: String,
        agent_type: AgentType,
        capabilities: Vec<String>,
        connection_metadata: ConnectionMetadata,
    ) -> Result<Self> {
        Self::validate_name(&name)?;
        Self::validate_capabilities(&capabilities)?;

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            agent_type,
            capabilities,
            status: AgentStatus::Connecting,
            connection_metadata,
            created_at: now,
            last_seen: now,
            performance_metrics: AgentPerformanceMetrics::default(),
            resource_allocation: ResourceAllocation::default(),
            health_info: HealthInfo::new(),
            system_prompt_assignment: None,
        })
    }

    /// Create a builder for constructing an Agent
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Validate agent name
    fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::validation("Agent name cannot be empty"));
        }
        if name.len() > 100 {
            return Err(Error::constraint_violation(
                "name_length",
                "Agent name cannot exceed 100 characters",
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(Error::validation(
                "Agent name can only contain alphanumeric characters, hyphens, and underscores",
            ));
        }
        Ok(())
    }

    /// Validate capabilities list
    fn validate_capabilities(capabilities: &[String]) -> Result<()> {
        if capabilities.is_empty() {
            return Err(Error::constraint_violation(
                "min_capabilities",
                "Agent must have at least one capability",
            ));
        }
        for capability in capabilities {
            if capability.trim().is_empty() {
                return Err(Error::validation("Capability cannot be empty"));
            }
            if capability.len() > 50 {
                return Err(Error::constraint_violation(
                    "capability_length",
                    "Capability name cannot exceed 50 characters",
                ));
            }
        }
        Ok(())
    }

    /// Update the agent's last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Utc::now();
    }

    /// Check if the agent is currently available for work
    pub fn is_available(&self) -> bool {
        matches!(self.status, AgentStatus::Online | AgentStatus::Idle)
            && self.resource_allocation.current_task_count
                < self.resource_allocation.max_concurrent_tasks
            && self
                .health_info
                .quarantine_until
                .map_or(true, |until| Utc::now() > until)
    }

    /// Update the agent's status
    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
        self.update_last_seen();
    }

    /// Add a capability to the agent
    pub fn add_capability(&mut self, capability: String) -> Result<()> {
        if capability.trim().is_empty() {
            return Err(Error::validation("Capability cannot be empty"));
        }
        if capability.len() > 50 {
            return Err(Error::constraint_violation(
                "capability_length",
                "Capability name cannot exceed 50 characters",
            ));
        }
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
            self.update_last_seen();
        }
        Ok(())
    }

    /// Remove a capability from the agent
    pub fn remove_capability(&mut self, capability: &str) -> Result<()> {
        if let Some(pos) = self.capabilities.iter().position(|c| c == capability) {
            if self.capabilities.len() == 1 {
                return Err(Error::constraint_violation(
                    "min_capabilities",
                    "Cannot remove the last capability - agent must have at least one capability",
                ));
            }
            self.capabilities.remove(pos);
            self.update_last_seen();
        }
        Ok(())
    }

    /// Check if the agent is healthy (recently seen and online)
    pub fn is_healthy(&self, max_idle_seconds: i64) -> bool {
        self.is_available()
            && Utc::now()
                .signed_duration_since(self.last_seen)
                .num_seconds()
                <= max_idle_seconds
    }

    /// Get the number of capabilities
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    /// Get all capabilities as a slice
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Check if the agent has all required capabilities
    pub fn has_all_capabilities(&self, required_capabilities: &[String]) -> bool {
        required_capabilities
            .iter()
            .all(|cap| self.has_capability(cap))
    }

    /// Check if the agent has any of the specified capabilities
    pub fn has_any_capability(&self, capabilities: &[String]) -> bool {
        capabilities.iter().any(|cap| self.has_capability(cap))
    }

    /// Check if the agent has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }

    /// Get the agent's uptime in seconds
    pub fn uptime_seconds(&self) -> i64 {
        self.last_seen
            .signed_duration_since(self.created_at)
            .num_seconds()
    }

    /// Transition agent to online state
    pub fn go_online(&mut self) -> Result<()> {
        match self.status {
            AgentStatus::Connecting | AgentStatus::Offline | AgentStatus::Idle => {
                self.status = AgentStatus::Online;
                self.update_last_seen();
                Ok(())
            }
            _ => Err(Error::state_transition(format!(
                "Cannot transition from {:?} to Online",
                self.status
            ))),
        }
    }

    /// Transition agent to busy state
    pub fn go_busy(&mut self) -> Result<()> {
        match self.status {
            AgentStatus::Online | AgentStatus::Idle => {
                self.status = AgentStatus::Busy;
                self.update_last_seen();
                Ok(())
            }
            _ => Err(Error::state_transition(format!(
                "Cannot transition from {:?} to Busy",
                self.status
            ))),
        }
    }

    /// Transition agent to idle state
    pub fn go_idle(&mut self) -> Result<()> {
        match self.status {
            AgentStatus::Online | AgentStatus::Busy => {
                self.status = AgentStatus::Idle;
                self.update_last_seen();
                Ok(())
            }
            _ => Err(Error::state_transition(format!(
                "Cannot transition from {:?} to Idle",
                self.status
            ))),
        }
    }

    /// Transition agent to maintenance mode
    pub fn enter_maintenance(&mut self, _reason: Option<String>) -> Result<()> {
        self.status = AgentStatus::Maintenance;
        self.update_last_seen();
        Ok(())
    }

    /// Exit maintenance mode
    pub fn exit_maintenance(&mut self) -> Result<()> {
        match self.status {
            AgentStatus::Maintenance => {
                self.status = AgentStatus::Online;
                self.update_last_seen();
                Ok(())
            }
            _ => Err(Error::state_transition(format!(
                "Cannot exit maintenance from {:?} state",
                self.status
            ))),
        }
    }

    /// Mark agent as disconnecting
    pub fn disconnect(&mut self) -> Result<()> {
        self.status = AgentStatus::Disconnecting;
        self.update_last_seen();
        Ok(())
    }

    /// Mark agent as offline
    pub fn go_offline(&mut self) -> Result<()> {
        self.status = AgentStatus::Offline;
        self.update_last_seen();
        // Reset resource allocation when going offline
        self.resource_allocation.current_task_count = 0;
        self.resource_allocation.calculate_load_factor();
        Ok(())
    }

    /// Record task assignment
    pub fn assign_task(&mut self) -> Result<()> {
        if !self.is_available() {
            return Err(Error::constraint_violation(
                "agent_availability",
                "Agent is not available for task assignment",
            ));
        }

        if !self.resource_allocation.assign_task() {
            return Err(Error::constraint_violation(
                "max_concurrent_tasks",
                "Agent has reached maximum concurrent task limit",
            ));
        }

        // If this is the first task and agent was idle, make it busy
        if self.resource_allocation.current_task_count == 1
            && matches!(self.status, AgentStatus::Idle)
        {
            self.go_busy()?;
        }

        self.update_last_seen();
        Ok(())
    }

    /// Record task completion
    pub fn complete_task(&mut self, response_time_ms: f64, success: bool) -> Result<()> {
        self.resource_allocation.complete_task();
        self.performance_metrics
            .record_task_completion(response_time_ms, success);

        // If no more tasks and agent was busy, make it online
        if self.resource_allocation.current_task_count == 0
            && matches!(self.status, AgentStatus::Busy)
        {
            self.status = AgentStatus::Online;
        }

        self.update_last_seen();
        Ok(())
    }

    /// Update agent performance metrics
    pub fn update_performance(&mut self, cpu_percent: f32, memory_percent: f32) {
        self.performance_metrics
            .update_resource_usage(cpu_percent, memory_percent);
        self.update_last_seen();
    }

    /// Record health check result
    pub fn record_health_check(&mut self, healthy: bool) {
        if healthy {
            self.health_info.record_healthy();
            // If agent was unhealthy, bring it back online
            if matches!(self.status, AgentStatus::Unhealthy { .. }) {
                let _ = self.go_online();
            }
        } else {
            self.health_info.record_failure();
            self.status = AgentStatus::Unhealthy {
                reason: format!(
                    "Failed {} consecutive health checks",
                    self.health_info.consecutive_failures
                ),
            };
        }
        self.update_last_seen();
    }

    /// Get agent score for load balancing (higher is better)
    pub fn calculate_load_balancing_score(&self) -> f64 {
        if !self.is_available() {
            return 0.0;
        }

        // Base score from success rate (0-100)
        let success_rate = self.performance_metrics.success_rate();

        // Penalty for high load (0-1, lower is better)
        let load_penalty = 1.0 - self.resource_allocation.load_factor as f64;

        // Bonus for priority level
        let priority_bonus = self.resource_allocation.priority_level as f64;

        // Penalty for high resource usage
        let resource_penalty = 1.0
            - (self.performance_metrics.cpu_usage_percent
                + self.performance_metrics.memory_usage_percent) as f64
                / 200.0;

        // Combine factors
        success_rate * load_penalty * priority_bonus * resource_penalty.max(0.1)
    }

    /// Check if agent matches capability requirements with weights
    pub fn matches_capabilities_weighted(
        &self,
        required_capabilities: &[(String, f32)],
    ) -> (bool, f32) {
        if required_capabilities.is_empty() {
            return (true, 1.0);
        }

        let mut total_weight = 0.0;
        let mut matched_weight = 0.0;

        for (capability, weight) in required_capabilities {
            total_weight += weight;
            if self.has_capability(capability) {
                matched_weight += weight;
            }
        }

        let match_ratio = if total_weight > 0.0 {
            matched_weight / total_weight
        } else {
            0.0
        };

        // Agent matches if it has at least 80% of weighted capabilities
        (match_ratio >= 0.8, match_ratio)
    }

    /// Assign a system prompt to the agent
    pub fn assign_system_prompt(
        &mut self,
        prompt_id: Uuid,
        prompt_version: String,
        context_data: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        self.system_prompt_assignment = Some(SystemPromptAssignment {
            prompt_id: Some(prompt_id),
            prompt_version: Some(prompt_version),
            assigned_at: Utc::now(),
            context_data,
        });
        self.update_last_seen();
        Ok(())
    }

    /// Clear the system prompt assignment
    pub fn clear_system_prompt(&mut self) {
        self.system_prompt_assignment = None;
        self.update_last_seen();
    }

    /// Check if agent has a system prompt assigned
    pub fn has_system_prompt(&self) -> bool {
        self.system_prompt_assignment.is_some()
    }

    /// Get the assigned system prompt ID
    pub fn get_system_prompt_id(&self) -> Option<Uuid> {
        self.system_prompt_assignment
            .as_ref()
            .and_then(|assignment| assignment.prompt_id)
    }

    /// Update context data for the assigned system prompt
    pub fn update_prompt_context(
        &mut self,
        context_data: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        if let Some(assignment) = &mut self.system_prompt_assignment {
            assignment.context_data = context_data;
            self.update_last_seen();
            Ok(())
        } else {
            Err(Error::constraint_violation(
                "no_prompt_assigned",
                "Cannot update context data: no system prompt is assigned to this agent",
            ))
        }
    }

    /// Get context data for the assigned system prompt
    pub fn get_prompt_context(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.system_prompt_assignment
            .as_ref()
            .map(|assignment| &assignment.context_data)
    }
}

/// Builder for constructing Agent instances with validation
#[derive(Debug, Clone)]
pub struct AgentBuilder {
    name: Option<String>,
    agent_type: Option<AgentType>,
    capabilities: Vec<String>,
    connection_metadata: Option<ConnectionMetadata>,
}

impl AgentBuilder {
    /// Create a new agent builder
    pub fn new() -> Self {
        Self {
            name: None,
            agent_type: None,
            capabilities: Vec::new(),
            connection_metadata: None,
        }
    }

    /// Set the agent name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the agent type
    pub fn agent_type(mut self, agent_type: AgentType) -> Self {
        self.agent_type = Some(agent_type);
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
        self.capabilities
            .extend(capabilities.into_iter().map(|c| c.into()));
        self
    }

    /// Set the connection metadata
    pub fn connection_metadata(mut self, metadata: ConnectionMetadata) -> Self {
        self.connection_metadata = Some(metadata);
        self
    }

    /// Build the Agent instance
    pub fn build(self) -> Result<Agent> {
        let name = self
            .name
            .ok_or_else(|| Error::validation("Agent name is required"))?;
        let agent_type = self
            .agent_type
            .ok_or_else(|| Error::validation("Agent type is required"))?;
        let connection_metadata = self
            .connection_metadata
            .ok_or_else(|| Error::validation("Connection metadata is required"))?;

        Agent::new(name, agent_type, self.capabilities, connection_metadata)
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionMetadata {
    /// Create a new connection metadata builder
    pub fn builder() -> ConnectionMetadataBuilder {
        ConnectionMetadataBuilder::new()
    }

    /// Create a new connection metadata instance with validation
    pub fn new(
        endpoint: String,
        protocol_version: String,
        session_id: Option<String>,
    ) -> Result<Self> {
        Self::validate_endpoint(&endpoint)?;
        Self::validate_protocol_version(&protocol_version)?;

        Ok(Self {
            endpoint,
            protocol_version,
            session_id,
            version: None,
            transport: None,
            capabilities: None,
            session_type: None,
            project_context: None,
            coordination_scope: None,
            specialization: None,
            coordinator_managed: None,
            workspace_isolation: None,
        })
    }

    /// Validate endpoint URL
    fn validate_endpoint(endpoint: &str) -> Result<()> {
        if endpoint.trim().is_empty() {
            return Err(Error::validation("Endpoint cannot be empty"));
        }
        // Basic URL validation - should start with http:// or https://
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(Error::validation(
                "Endpoint must be a valid HTTP or HTTPS URL",
            ));
        }
        if endpoint.len() > 500 {
            return Err(Error::constraint_violation(
                "endpoint_length",
                "Endpoint URL cannot exceed 500 characters",
            ));
        }
        Ok(())
    }

    /// Validate protocol version
    fn validate_protocol_version(version: &str) -> Result<()> {
        if version.trim().is_empty() {
            return Err(Error::validation("Protocol version cannot be empty"));
        }
        if version.len() > 20 {
            return Err(Error::constraint_violation(
                "version_length",
                "Protocol version cannot exceed 20 characters",
            ));
        }
        Ok(())
    }
}

/// Builder for constructing ConnectionMetadata instances
#[derive(Debug, Clone)]
pub struct ConnectionMetadataBuilder {
    endpoint: Option<String>,
    protocol_version: Option<String>,
    session_id: Option<String>,
}

impl ConnectionMetadataBuilder {
    /// Create a new connection metadata builder
    pub fn new() -> Self {
        Self {
            endpoint: None,
            protocol_version: None,
            session_id: None,
        }
    }

    /// Set the endpoint
    pub fn endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the protocol version
    pub fn protocol_version<S: Into<String>>(mut self, version: S) -> Self {
        self.protocol_version = Some(version.into());
        self
    }

    /// Set the session ID
    pub fn session_id<S: Into<String>>(mut self, session_id: S) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Build the ConnectionMetadata instance
    pub fn build(self) -> Result<ConnectionMetadata> {
        let endpoint = self
            .endpoint
            .ok_or_else(|| Error::validation("Endpoint is required"))?;
        let protocol_version = self
            .protocol_version
            .ok_or_else(|| Error::validation("Protocol version is required"))?;

        ConnectionMetadata::new(endpoint, protocol_version, self.session_id)
    }
}

impl Default for ConnectionMetadataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AgentPerformanceMetrics {
    fn default() -> Self {
        Self {
            tasks_completed: 0,
            tasks_failed: 0,
            average_response_time_ms: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_percent: 0.0,
            last_task_completed_at: None,
        }
    }
}

impl AgentPerformanceMetrics {
    /// Update performance metrics after task completion
    pub fn record_task_completion(&mut self, response_time_ms: f64, success: bool) {
        if success {
            self.tasks_completed += 1;
        } else {
            self.tasks_failed += 1;
        }

        // Update rolling average response time
        let total_tasks = self.tasks_completed + self.tasks_failed;
        if total_tasks > 0 {
            self.average_response_time_ms =
                (self.average_response_time_ms * (total_tasks - 1) as f64 + response_time_ms)
                    / total_tasks as f64;
        }

        self.last_task_completed_at = Some(Utc::now());
    }

    /// Update system resource usage
    pub fn update_resource_usage(&mut self, cpu_percent: f32, memory_percent: f32) {
        self.cpu_usage_percent = cpu_percent;
        self.memory_usage_percent = memory_percent;
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            100.0
        } else {
            (self.tasks_completed as f64 / total as f64) * 100.0
        }
    }
}

impl Default for ResourceAllocation {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 5, // Default to 5 concurrent tasks
            current_task_count: 0,
            load_factor: 0.0,
            priority_level: 1, // Default priority
        }
    }
}

impl ResourceAllocation {
    /// Calculate current load factor (0.0 to 1.0)
    pub fn calculate_load_factor(&mut self) {
        self.load_factor = if self.max_concurrent_tasks == 0 {
            0.0
        } else {
            self.current_task_count as f32 / self.max_concurrent_tasks as f32
        };
    }

    /// Check if agent can take on more tasks
    pub fn can_accept_task(&self) -> bool {
        self.current_task_count < self.max_concurrent_tasks
    }

    /// Increment current task count
    pub fn assign_task(&mut self) -> bool {
        if self.can_accept_task() {
            self.current_task_count += 1;
            self.calculate_load_factor();
            true
        } else {
            false
        }
    }

    /// Decrement current task count
    pub fn complete_task(&mut self) {
        if self.current_task_count > 0 {
            self.current_task_count -= 1;
            self.calculate_load_factor();
        }
    }

    /// Set maximum concurrent tasks
    pub fn set_max_concurrent_tasks(&mut self, max_tasks: u32) {
        self.max_concurrent_tasks = max_tasks;
        self.calculate_load_factor();
    }
}

impl HealthInfo {
    /// Create new health info with current timestamp
    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_health_check: Utc::now(),
            recovery_attempts: 0,
            quarantine_until: None,
        }
    }

    /// Record a successful health check
    pub fn record_healthy(&mut self) {
        self.consecutive_failures = 0;
        self.last_health_check = Utc::now();
        self.quarantine_until = None;
    }

    /// Record a failed health check
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_health_check = Utc::now();

        // If we have too many consecutive failures, quarantine the agent
        if self.consecutive_failures >= 3 {
            let quarantine_duration = std::cmp::min(self.recovery_attempts * 30, 300); // Max 5 minutes
            self.quarantine_until =
                Some(Utc::now() + chrono::Duration::seconds(quarantine_duration as i64));
            self.recovery_attempts += 1;
        }
    }

    /// Check if agent is currently quarantined
    pub fn is_quarantined(&self) -> bool {
        self.quarantine_until
            .is_some_and(|until| Utc::now() < until)
    }

    /// Get time until quarantine ends
    pub fn quarantine_remaining_seconds(&self) -> Option<i64> {
        self.quarantine_until.map(|until| {
            let remaining = until.signed_duration_since(Utc::now()).num_seconds();
            std::cmp::max(0, remaining)
        })
    }
}

impl Default for HealthInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation_with_builder() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .capability("validation")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.agent_type, AgentType::Worker);
        assert_eq!(agent.capabilities.len(), 2);
        assert!(agent.has_capability("testing"));
        assert!(agent.has_capability("validation"));

        // Agent starts in Connecting state, transition to Online to be available
        assert_eq!(agent.status, AgentStatus::Connecting);
        agent.go_online().unwrap();
        assert!(agent.is_available());
    }

    #[test]
    fn test_agent_name_validation() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        // Empty name should fail
        let result = Agent::builder()
            .name("")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata.clone())
            .build();
        assert!(result.is_err());

        // Invalid characters should fail
        let result = Agent::builder()
            .name("test@agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata.clone())
            .build();
        assert!(result.is_err());

        // Too long name should fail
        let long_name = "a".repeat(101);
        let result = Agent::builder()
            .name(long_name)
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_capabilities_validation() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        // No capabilities should fail
        let result = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .connection_metadata(metadata)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_connection_metadata_validation() {
        // Invalid endpoint should fail
        let result = ConnectionMetadata::builder()
            .endpoint("invalid-url")
            .protocol_version("1.0")
            .build();
        assert!(result.is_err());

        // Empty protocol version should fail
        let result = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("")
            .build();
        assert!(result.is_err());

        // Valid metadata should succeed
        let result = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .session_id("session-123")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_status_operations() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        // Agent starts in Connecting state, transition to Online
        agent.go_online().unwrap();
        assert!(agent.is_available());

        agent.set_status(AgentStatus::Busy);
        assert!(!agent.is_available());
        assert_eq!(agent.status, AgentStatus::Busy);

        agent.set_status(AgentStatus::Error {
            message: "Test error".to_string(),
        });
        assert!(!agent.is_available());
    }

    #[test]
    fn test_agent_capability_operations() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        assert!(agent.has_capability("testing"));
        assert!(!agent.has_capability("nonexistent"));

        agent.add_capability("new-capability".to_string()).unwrap();
        assert!(agent.has_capability("new-capability"));

        // Adding empty capability should fail
        let result = agent.add_capability("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_uptime() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        let uptime = agent.uptime_seconds();
        assert!(uptime >= 0);
    }

    #[test]
    fn test_agent_update_last_seen() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        let initial_last_seen = agent.last_seen;
        std::thread::sleep(std::time::Duration::from_millis(10));
        agent.update_last_seen();
        assert!(agent.last_seen > initial_last_seen);
    }

    #[test]
    fn test_agent_capability_removal() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .capability("validation")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        assert_eq!(agent.capability_count(), 2);

        // Remove one capability
        agent.remove_capability("testing").unwrap();
        assert_eq!(agent.capability_count(), 1);
        assert!(!agent.has_capability("testing"));
        assert!(agent.has_capability("validation"));

        // Cannot remove the last capability
        let result = agent.remove_capability("validation");
        assert!(result.is_err());
        assert_eq!(agent.capability_count(), 1);
    }

    #[test]
    fn test_agent_health_check() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        // Agent starts in Connecting state, transition to Online
        agent.go_online().unwrap();
        // Fresh agent should be healthy
        assert!(agent.is_healthy(60));

        // Offline agent should not be healthy
        agent.set_status(AgentStatus::Offline);
        assert!(!agent.is_healthy(60));

        // Back online but with negative max idle (simulating old timestamp)
        agent.set_status(AgentStatus::Online);
        assert!(!agent.is_healthy(-1));
    }

    #[test]
    fn test_agent_capability_queries() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("rust")
            .capability("testing")
            .capability("validation")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        // Test has_all_capabilities
        assert!(agent.has_all_capabilities(&["rust".to_string(), "testing".to_string()]));
        assert!(!agent.has_all_capabilities(&["rust".to_string(), "python".to_string()]));

        // Test has_any_capability
        assert!(agent.has_any_capability(&["python".to_string(), "testing".to_string()]));
        assert!(!agent.has_any_capability(&["python".to_string(), "java".to_string()]));

        // Test capabilities access
        let caps = agent.capabilities();
        assert_eq!(caps.len(), 3);
        assert!(caps.contains(&"rust".to_string()));
    }

    #[test]
    fn test_agent_capability_length_validation() {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let mut agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        // Too long capability should fail
        let long_capability = "a".repeat(51);
        let result = agent.add_capability(long_capability);
        assert!(result.is_err());
    }

    #[test]
    fn test_connection_metadata_validation_enhancements() {
        // Too long endpoint should fail
        let long_endpoint = format!("https://{}.com", "a".repeat(500));
        let result = ConnectionMetadata::builder()
            .endpoint(long_endpoint)
            .protocol_version("1.0")
            .build();
        assert!(result.is_err());

        // Too long protocol version should fail
        let long_version = "a".repeat(21);
        let result = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version(long_version)
            .build();
        assert!(result.is_err());
    }
}
