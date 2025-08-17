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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{Error, Result};

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
    Online,
    Offline,
    Busy,
    Error { message: String },
}

/// Connection metadata for an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectionMetadata {
    pub endpoint: String,
    pub protocol_version: String,
    pub session_id: Option<String>,
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
            status: AgentStatus::Online,
            connection_metadata,
            created_at: now,
            last_seen: now,
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
                "Agent name cannot exceed 100 characters"
            ));
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(Error::validation(
                "Agent name can only contain alphanumeric characters, hyphens, and underscores"
            ));
        }
        Ok(())
    }

    /// Validate capabilities list
    fn validate_capabilities(capabilities: &[String]) -> Result<()> {
        if capabilities.is_empty() {
            return Err(Error::constraint_violation(
                "min_capabilities",
                "Agent must have at least one capability"
            ));
        }
        for capability in capabilities {
            if capability.trim().is_empty() {
                return Err(Error::validation("Capability cannot be empty"));
            }
            if capability.len() > 50 {
                return Err(Error::constraint_violation(
                    "capability_length",
                    "Capability name cannot exceed 50 characters"
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
        matches!(self.status, AgentStatus::Online)
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
                "Capability name cannot exceed 50 characters"
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
                    "Cannot remove the last capability - agent must have at least one capability"
                ));
            }
            self.capabilities.remove(pos);
            self.update_last_seen();
        }
        Ok(())
    }

    /// Check if the agent is healthy (recently seen and online)
    pub fn is_healthy(&self, max_idle_seconds: i64) -> bool {
        self.is_available() && 
            Utc::now().signed_duration_since(self.last_seen).num_seconds() <= max_idle_seconds
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
        required_capabilities.iter().all(|cap| self.has_capability(cap))
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
        self.last_seen.signed_duration_since(self.created_at).num_seconds()
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
        self.capabilities.extend(capabilities.into_iter().map(|c| c.into()));
        self
    }

    /// Set the connection metadata
    pub fn connection_metadata(mut self, metadata: ConnectionMetadata) -> Self {
        self.connection_metadata = Some(metadata);
        self
    }

    /// Build the Agent instance
    pub fn build(self) -> Result<Agent> {
        let name = self.name.ok_or_else(|| Error::validation("Agent name is required"))?;
        let agent_type = self.agent_type.ok_or_else(|| Error::validation("Agent type is required"))?;
        let connection_metadata = self.connection_metadata.ok_or_else(|| Error::validation("Connection metadata is required"))?;

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
        })
    }

    /// Validate endpoint URL
    fn validate_endpoint(endpoint: &str) -> Result<()> {
        if endpoint.trim().is_empty() {
            return Err(Error::validation("Endpoint cannot be empty"));
        }
        // Basic URL validation - should start with http:// or https://
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(Error::validation("Endpoint must be a valid HTTP or HTTPS URL"));
        }
        if endpoint.len() > 500 {
            return Err(Error::constraint_violation(
                "endpoint_length",
                "Endpoint URL cannot exceed 500 characters"
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
                "Protocol version cannot exceed 20 characters"
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
        let endpoint = self.endpoint.ok_or_else(|| Error::validation("Endpoint is required"))?;
        let protocol_version = self.protocol_version.ok_or_else(|| Error::validation("Protocol version is required"))?;

        ConnectionMetadata::new(endpoint, protocol_version, self.session_id)
    }
}

impl Default for ConnectionMetadataBuilder {
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

        let agent = Agent::builder()
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