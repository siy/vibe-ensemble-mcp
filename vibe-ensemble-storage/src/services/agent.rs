//! Agent service for business logic and coordination

use crate::{repositories::AgentRepository, Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::agent::{Agent, AgentStatus, AgentType, ConnectionMetadata};

/// Service for managing agent registration and coordination
pub struct AgentService {
    repository: Arc<AgentRepository>,
    /// In-memory session tracking for active connections
    active_sessions: Arc<RwLock<HashMap<Uuid, AgentSession>>>,
}

/// Active agent session information
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub agent_id: Uuid,
    pub session_id: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

impl AgentService {
    /// Create a new agent service
    pub fn new(repository: Arc<AgentRepository>) -> Self {
        Self {
            repository,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new agent
    pub async fn register_agent(
        &self,
        name: String,
        agent_type: AgentType,
        capabilities: Vec<String>,
        connection_metadata: ConnectionMetadata,
        session_id: String,
    ) -> Result<Agent> {
        info!("Registering new agent: {} (type: {:?})", name, agent_type);

        // Check if agent name already exists
        if let Some(_existing) = self.repository.find_by_name(&name).await? {
            return Err(Error::Conflict(format!(
                "Agent with name '{}' already exists",
                name
            )));
        }

        // Create the agent
        let agent = Agent::new(name, agent_type, capabilities, connection_metadata)?;

        // Store in database
        self.repository.create(&agent).await?;

        // Create active session
        let session = AgentSession {
            agent_id: agent.id,
            session_id: session_id.clone(),
            connected_at: chrono::Utc::now(),
            last_heartbeat: chrono::Utc::now(),
        };

        self.active_sessions.write().await.insert(agent.id, session);

        info!(
            "Successfully registered agent: {} ({})",
            agent.name, agent.id
        );
        Ok(agent)
    }

    /// Deregister an agent (on disconnect)
    pub async fn deregister_agent(&self, agent_id: Uuid) -> Result<()> {
        info!("Deregistering agent: {}", agent_id);

        // Update agent status to offline
        self.repository
            .update_status(agent_id, &AgentStatus::Offline)
            .await?;

        // Remove from active sessions
        self.active_sessions.write().await.remove(&agent_id);

        info!("Successfully deregistered agent: {}", agent_id);
        Ok(())
    }

    /// Update agent heartbeat (health check)
    pub async fn update_heartbeat(&self, agent_id: Uuid) -> Result<()> {
        debug!("Updating heartbeat for agent: {}", agent_id);

        // Update last seen in database
        self.repository.update_last_seen(agent_id).await?;

        // Update active session heartbeat
        if let Some(session) = self.active_sessions.write().await.get_mut(&agent_id) {
            session.last_heartbeat = chrono::Utc::now();
        }

        Ok(())
    }

    /// Update agent status
    pub async fn update_agent_status(&self, agent_id: Uuid, status: AgentStatus) -> Result<()> {
        info!("Updating agent status: {} -> {:?}", agent_id, status);

        self.repository.update_status(agent_id, &status).await?;

        // If agent goes offline, remove from active sessions
        if matches!(status, AgentStatus::Offline) {
            self.active_sessions.write().await.remove(&agent_id);
        }

        Ok(())
    }

    /// Get agent by ID
    pub async fn get_agent(&self, agent_id: Uuid) -> Result<Option<Agent>> {
        self.repository.find_by_id(agent_id).await
    }

    /// Get agent by name
    pub async fn get_agent_by_name(&self, name: &str) -> Result<Option<Agent>> {
        self.repository.find_by_name(name).await
    }

    /// List all agents
    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        self.repository.list().await
    }

    /// List online agents only
    pub async fn list_online_agents(&self) -> Result<Vec<Agent>> {
        self.repository.list_by_status(&AgentStatus::Online).await
    }

    /// List agents by type
    pub async fn list_agents_by_type(&self, agent_type: &AgentType) -> Result<Vec<Agent>> {
        self.repository.list_by_type(agent_type).await
    }

    /// Find agents with specific capability
    pub async fn find_agents_by_capability(&self, capability: &str) -> Result<Vec<Agent>> {
        self.repository.find_by_capability(capability).await
    }

    /// Find available agents with specific capability
    pub async fn find_available_agents_by_capability(
        &self,
        capability: &str,
    ) -> Result<Vec<Agent>> {
        let all_agents = self.repository.find_by_capability(capability).await?;
        Ok(all_agents
            .into_iter()
            .filter(|agent| agent.is_available())
            .collect())
    }

    /// Get agent statistics
    pub async fn get_statistics(&self) -> Result<AgentStatistics> {
        let total_count = self.repository.count().await?;
        let online_count = self
            .repository
            .count_by_status(&AgentStatus::Online)
            .await?;
        let busy_count = self.repository.count_by_status(&AgentStatus::Busy).await?;
        let offline_count = self
            .repository
            .count_by_status(&AgentStatus::Offline)
            .await?;

        let coordinator_count = self
            .repository
            .list_by_type(&AgentType::Coordinator)
            .await?
            .len() as i64;
        let worker_count = self
            .repository
            .list_by_type(&AgentType::Worker)
            .await?
            .len() as i64;

        let active_sessions = self.active_sessions.read().await.len() as i64;

        Ok(AgentStatistics {
            total_agents: total_count,
            online_agents: online_count,
            busy_agents: busy_count,
            offline_agents: offline_count,
            coordinator_agents: coordinator_count,
            worker_agents: worker_count,
            active_sessions,
        })
    }

    /// Check for unhealthy agents (haven't sent heartbeat recently)
    pub async fn check_agent_health(&self, max_idle_seconds: i64) -> Result<Vec<Uuid>> {
        debug!(
            "Checking agent health with max idle: {} seconds",
            max_idle_seconds
        );

        let agents = self.repository.list_by_status(&AgentStatus::Online).await?;
        let mut unhealthy_agents = Vec::new();

        for agent in agents {
            if !agent.is_healthy(max_idle_seconds) {
                warn!(
                    "Agent {} is unhealthy (last seen: {})",
                    agent.id, agent.last_seen
                );
                unhealthy_agents.push(agent.id);

                // Mark as offline
                self.update_agent_status(agent.id, AgentStatus::Offline)
                    .await?;
            }
        }

        if !unhealthy_agents.is_empty() {
            info!("Found {} unhealthy agents", unhealthy_agents.len());
        }

        Ok(unhealthy_agents)
    }

    /// Get active session information
    pub async fn get_active_sessions(&self) -> Vec<AgentSession> {
        self.active_sessions
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Check if agent is actively connected
    pub async fn is_agent_connected(&self, agent_id: Uuid) -> bool {
        self.active_sessions.read().await.contains_key(&agent_id)
    }

    /// Get session by agent ID
    pub async fn get_session(&self, agent_id: Uuid) -> Option<AgentSession> {
        self.active_sessions.read().await.get(&agent_id).cloned()
    }

    /// Clean up stale sessions (sessions without recent heartbeat)
    pub async fn cleanup_stale_sessions(&self, max_session_idle_seconds: i64) -> Result<Vec<Uuid>> {
        debug!(
            "Cleaning up stale sessions with max idle: {} seconds",
            max_session_idle_seconds
        );

        let now = chrono::Utc::now();
        let mut stale_sessions = Vec::new();
        let mut sessions = self.active_sessions.write().await;

        let agent_ids_to_remove: Vec<Uuid> = sessions
            .iter()
            .filter_map(|(agent_id, session)| {
                let idle_duration = now.signed_duration_since(session.last_heartbeat);
                if idle_duration.num_seconds() > max_session_idle_seconds {
                    Some(*agent_id)
                } else {
                    None
                }
            })
            .collect();

        for agent_id in agent_ids_to_remove {
            if let Some(session) = sessions.remove(&agent_id) {
                warn!(
                    "Removing stale session for agent: {} (last heartbeat: {})",
                    agent_id, session.last_heartbeat
                );
                stale_sessions.push(agent_id);

                // Update agent status to offline
                if let Err(e) = self
                    .repository
                    .update_status(agent_id, &AgentStatus::Offline)
                    .await
                {
                    warn!(
                        "Failed to update status for stale agent {}: {}",
                        agent_id, e
                    );
                }
            }
        }

        if !stale_sessions.is_empty() {
            info!("Cleaned up {} stale sessions", stale_sessions.len());
        }

        Ok(stale_sessions)
    }
}

/// Agent system statistics
#[derive(Debug, Clone)]
pub struct AgentStatistics {
    pub total_agents: i64,
    pub online_agents: i64,
    pub busy_agents: i64,
    pub offline_agents: i64,
    pub coordinator_agents: i64,
    pub worker_agents: i64,
    pub active_sessions: i64,
}

#[cfg(test)]
mod tests {
    include!("agent_tests.rs");
}
