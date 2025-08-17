//! Agent domain model and related types

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
    /// Create a new agent instance
    pub fn new(
        name: String,
        agent_type: AgentType,
        capabilities: Vec<String>,
        connection_metadata: ConnectionMetadata,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            agent_type,
            capabilities,
            status: AgentStatus::Online,
            connection_metadata,
            created_at: now,
            last_seen: now,
        }
    }

    /// Update the agent's last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Utc::now();
    }

    /// Check if the agent is currently available for work
    pub fn is_available(&self) -> bool {
        matches!(self.status, AgentStatus::Online)
    }
}