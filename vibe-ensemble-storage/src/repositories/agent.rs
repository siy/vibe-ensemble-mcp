//! Agent repository implementation

use crate::{Error, Result};
use anyhow;
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::{Pool, Sqlite};
use tracing::{debug, info};
use uuid::Uuid;
use vibe_ensemble_core::agent::{Agent, AgentStatus, AgentType, ConnectionMetadata};

/// Repository for agent entities
pub struct AgentRepository {
    pool: Pool<Sqlite>,
}

impl AgentRepository {
    /// Create a new agent repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new agent
    pub async fn create(&self, agent: &Agent) -> Result<()> {
        debug!("Creating agent: {} ({})", agent.name, agent.id);

        let capabilities_json = serde_json::to_string(&agent.capabilities).map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to serialize capabilities: {}", e))
        })?;

        let status_json = serde_json::to_string(&agent.status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize status: {}", e)))?;

        let connection_metadata_json =
            serde_json::to_string(&agent.connection_metadata).map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to serialize connection metadata: {}",
                    e
                ))
            })?;

        let agent_type_str = match agent.agent_type {
            AgentType::Coordinator => "Coordinator",
            AgentType::Worker => "Worker",
        };

        let agent_id_str = agent.id.to_string();
        let created_at_str = agent.created_at.to_rfc3339();
        let last_seen_str = agent.last_seen.to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO agents (id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            agent_id_str,
            agent.name,
            agent_type_str,
            capabilities_json,
            status_json,
            connection_metadata_json,
            created_at_str,
            last_seen_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        info!("Successfully created agent: {} ({})", agent.name, agent.id);
        Ok(())
    }

    /// Find an agent by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Agent>> {
        debug!("Finding agent by ID: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents WHERE id = ?1",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let agent = self.parse_agent_from_row(
                    row.id.as_ref().unwrap(),
                    &row.name,
                    &row.agent_type,
                    &row.capabilities,
                    &row.status,
                    &row.connection_metadata,
                    &row.created_at,
                    &row.last_seen,
                )?;
                Ok(Some(agent))
            }
            None => Ok(None),
        }
    }

    /// Find an agent by name
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Agent>> {
        debug!("Finding agent by name: {}", name);

        let row = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents WHERE name = ?1",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let agent = self.parse_agent_from_row(
                    row.id.as_ref().unwrap(),
                    &row.name,
                    &row.agent_type,
                    &row.capabilities,
                    &row.status,
                    &row.connection_metadata,
                    &row.created_at,
                    &row.last_seen,
                )?;
                Ok(Some(agent))
            }
            None => Ok(None),
        }
    }

    /// Update an agent
    pub async fn update(&self, agent: &Agent) -> Result<()> {
        debug!("Updating agent: {} ({})", agent.name, agent.id);

        let capabilities_json = serde_json::to_string(&agent.capabilities).map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to serialize capabilities: {}", e))
        })?;

        let status_json = serde_json::to_string(&agent.status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize status: {}", e)))?;

        let connection_metadata_json =
            serde_json::to_string(&agent.connection_metadata).map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to serialize connection metadata: {}",
                    e
                ))
            })?;

        let agent_type_str = match agent.agent_type {
            AgentType::Coordinator => "Coordinator",
            AgentType::Worker => "Worker",
        };

        let agent_id_str = agent.id.to_string();
        let last_seen_str = agent.last_seen.to_rfc3339();

        let result = sqlx::query!(
            r#"
            UPDATE agents 
            SET name = ?2, agent_type = ?3, capabilities = ?4, status = ?5, 
                connection_metadata = ?6, last_seen = ?7
            WHERE id = ?1
            "#,
            agent_id_str,
            agent.name,
            agent_type_str,
            capabilities_json,
            status_json,
            connection_metadata_json,
            last_seen_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::NotFound {
                entity: "Agent".to_string(),
                id: agent.id.to_string(),
            });
        }

        info!("Successfully updated agent: {} ({})", agent.name, agent.id);
        Ok(())
    }

    /// Update agent status
    pub async fn update_status(&self, id: Uuid, status: &AgentStatus) -> Result<()> {
        debug!("Updating agent status: {} -> {:?}", id, status);

        let status_json = serde_json::to_string(status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize status: {}", e)))?;

        let id_str = id.to_string();
        let last_seen_str = Utc::now().to_rfc3339();

        let result = sqlx::query!(
            "UPDATE agents SET status = ?1, last_seen = ?2 WHERE id = ?3",
            status_json,
            last_seen_str,
            id_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::NotFound {
                entity: "Agent".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }

    /// Update agent last seen timestamp
    pub async fn update_last_seen(&self, id: Uuid) -> Result<()> {
        debug!("Updating agent last seen: {}", id);

        let id_str = id.to_string();
        let last_seen_str = Utc::now().to_rfc3339();

        let result = sqlx::query!(
            "UPDATE agents SET last_seen = ?1 WHERE id = ?2",
            last_seen_str,
            id_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::NotFound {
                entity: "Agent".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }

    /// Delete an agent
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        debug!("Deleting agent: {}", id);

        let id_str = id.to_string();
        let result = sqlx::query!("DELETE FROM agents WHERE id = ?1", id_str)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::NotFound {
                entity: "Agent".to_string(),
                id: id.to_string(),
            });
        }

        info!("Successfully deleted agent: {}", id);
        Ok(())
    }

    /// List all agents
    pub async fn list(&self) -> Result<Vec<Agent>> {
        debug!("Listing all agents");

        let rows = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut agents = Vec::new();
        for row in rows {
            let agent = self.parse_agent_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                &row.agent_type,
                &row.capabilities,
                &row.status,
                &row.connection_metadata,
                &row.created_at,
                &row.last_seen,
            )?;
            agents.push(agent);
        }

        debug!("Found {} agents", agents.len());
        Ok(agents)
    }

    /// List agents by status
    pub async fn list_by_status(&self, status: &AgentStatus) -> Result<Vec<Agent>> {
        debug!("Listing agents by status: {:?}", status);

        let status_json = serde_json::to_string(status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize status: {}", e)))?;

        let rows = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents WHERE status = ?1 ORDER BY created_at DESC",
            status_json
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut agents = Vec::new();
        for row in rows {
            let agent = self.parse_agent_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                &row.agent_type,
                &row.capabilities,
                &row.status,
                &row.connection_metadata,
                &row.created_at,
                &row.last_seen,
            )?;
            agents.push(agent);
        }

        debug!("Found {} agents with status {:?}", agents.len(), status);
        Ok(agents)
    }

    /// List agents by type
    pub async fn list_by_type(&self, agent_type: &AgentType) -> Result<Vec<Agent>> {
        debug!("Listing agents by type: {:?}", agent_type);

        let agent_type_str = match agent_type {
            AgentType::Coordinator => "Coordinator",
            AgentType::Worker => "Worker",
        };

        let rows = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents WHERE agent_type = ?1 ORDER BY created_at DESC",
            agent_type_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut agents = Vec::new();
        for row in rows {
            let agent = self.parse_agent_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                &row.agent_type,
                &row.capabilities,
                &row.status,
                &row.connection_metadata,
                &row.created_at,
                &row.last_seen,
            )?;
            agents.push(agent);
        }

        debug!("Found {} agents with type {:?}", agents.len(), agent_type);
        Ok(agents)
    }

    /// Find agents by capability
    pub async fn find_by_capability(&self, capability: &str) -> Result<Vec<Agent>> {
        debug!("Finding agents with capability: {}", capability);

        let rows = sqlx::query!(
            "SELECT id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen FROM agents ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut agents = Vec::new();
        for row in rows {
            let agent = self.parse_agent_from_row(
                row.id.as_ref().unwrap(),
                &row.name,
                &row.agent_type,
                &row.capabilities,
                &row.status,
                &row.connection_metadata,
                &row.created_at,
                &row.last_seen,
            )?;
            if agent.has_capability(capability) {
                agents.push(agent);
            }
        }

        debug!(
            "Found {} agents with capability: {}",
            agents.len(),
            capability
        );
        Ok(agents)
    }

    /// Count agents
    pub async fn count(&self) -> Result<i64> {
        debug!("Counting agents");

        let row = sqlx::query!("SELECT COUNT(*) as count FROM agents")
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(row.count.into())
    }

    /// Count agents by status
    pub async fn count_by_status(&self, status: &AgentStatus) -> Result<i64> {
        debug!("Counting agents by status: {:?}", status);

        let status_json = serde_json::to_string(status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize status: {}", e)))?;

        let row = sqlx::query!(
            "SELECT COUNT(*) as count FROM agents WHERE status = ?1",
            status_json
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(row.count.into())
    }

    /// Check if agent exists
    pub async fn exists(&self, id: Uuid) -> Result<bool> {
        debug!("Checking if agent exists: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!("SELECT COUNT(*) as count FROM agents WHERE id = ?1", id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(row.count > 0)
    }

    /// Helper method to parse agent from database fields
    fn parse_agent_from_row(
        &self,
        id: &str,
        name: &str,
        agent_type: &str,
        capabilities: &str,
        status: &str,
        connection_metadata: &str,
        created_at: &str,
        last_seen: &str,
    ) -> Result<Agent> {
        let parsed_id = Uuid::parse_str(id)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Invalid agent UUID: {}", e)))?;

        let parsed_agent_type = match agent_type {
            "Coordinator" => AgentType::Coordinator,
            "Worker" => AgentType::Worker,
            _ => {
                return Err(Error::Internal(anyhow::anyhow!(
                    "Invalid agent type: {}",
                    agent_type
                )))
            }
        };

        let parsed_capabilities: Vec<String> = serde_json::from_str(capabilities).map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to deserialize capabilities: {}", e))
        })?;

        let parsed_status: AgentStatus = serde_json::from_str(status)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to deserialize status: {}", e)))?;

        let parsed_connection_metadata: ConnectionMetadata =
            serde_json::from_str(connection_metadata).map_err(|e| {
                Error::Internal(anyhow::anyhow!(
                    "Failed to deserialize connection metadata: {}",
                    e
                ))
            })?;

        let parsed_created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse created_at: {}", e)))?
            .with_timezone(&Utc);

        let parsed_last_seen = DateTime::parse_from_rfc3339(last_seen)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse last_seen: {}", e)))?
            .with_timezone(&Utc);

        Ok(Agent {
            id: parsed_id,
            name: name.to_string(),
            agent_type: parsed_agent_type,
            capabilities: parsed_capabilities,
            status: parsed_status,
            connection_metadata: parsed_connection_metadata,
            created_at: parsed_created_at,
            last_seen: parsed_last_seen,
            performance_metrics: vibe_ensemble_core::agent::AgentPerformanceMetrics::default(),
            resource_allocation: vibe_ensemble_core::agent::ResourceAllocation::default(),
            health_info: vibe_ensemble_core::agent::HealthInfo::default(),
            system_prompt_assignment: None,
        })
    }
}

#[cfg(test)]
mod tests {
    include!("agent_tests.rs");
}
