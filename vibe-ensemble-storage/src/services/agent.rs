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
        let mut agent = Agent::new(name, agent_type, capabilities, connection_metadata)?;

        // Transition to online after successful connection
        agent.go_online()?;

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

    /// Find best agent for a task with weighted capability requirements
    pub async fn find_best_agent_for_task(
        &self,
        required_capabilities: &[(String, f32)],
        exclude_agents: Option<&[Uuid]>,
    ) -> Result<Option<Agent>> {
        info!(
            "Finding best agent for task with {} capability requirements",
            required_capabilities.len()
        );

        let agents = self.list_online_agents().await?;
        let exclude_set: std::collections::HashSet<Uuid> =
            exclude_agents.unwrap_or(&[]).iter().copied().collect();

        let mut candidates: Vec<(Agent, f64)> = agents
            .into_iter()
            .filter(|agent| !exclude_set.contains(&agent.id))
            .filter_map(|agent| {
                let (matches, capability_score) =
                    agent.matches_capabilities_weighted(required_capabilities);
                if matches && agent.is_available() {
                    let load_balancing_score = agent.calculate_load_balancing_score();
                    let combined_score = capability_score as f64 * 0.6 + load_balancing_score * 0.4;
                    Some((agent, combined_score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        match candidates.first() {
            Some((agent, score)) => {
                info!(
                    "Selected agent {} (score: {:.2}) for task",
                    agent.name, score
                );
                Ok(Some(agent.clone()))
            }
            None => {
                warn!("No suitable agent found for task with given requirements");
                Ok(None)
            }
        }
    }

    /// Find multiple agents for a task requiring parallel execution
    pub async fn find_agents_for_parallel_task(
        &self,
        required_capabilities: &[(String, f32)],
        target_count: usize,
        exclude_agents: Option<&[Uuid]>,
    ) -> Result<Vec<Agent>> {
        info!(
            "Finding {} agents for parallel task execution",
            target_count
        );

        let agents = self.list_online_agents().await?;
        let exclude_set: std::collections::HashSet<Uuid> =
            exclude_agents.unwrap_or(&[]).iter().copied().collect();

        let mut candidates: Vec<(Agent, f64)> = agents
            .into_iter()
            .filter(|agent| !exclude_set.contains(&agent.id))
            .filter_map(|agent| {
                let (matches, capability_score) =
                    agent.matches_capabilities_weighted(required_capabilities);
                if matches && agent.is_available() {
                    let load_balancing_score = agent.calculate_load_balancing_score();
                    let combined_score = capability_score as f64 * 0.6 + load_balancing_score * 0.4;
                    Some((agent, combined_score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let selected: Vec<Agent> = candidates
            .into_iter()
            .take(target_count)
            .map(|(agent, _)| agent)
            .collect();

        info!(
            "Selected {} agents for parallel task execution",
            selected.len()
        );
        Ok(selected)
    }

    /// Get capability statistics across all agents
    pub async fn get_capability_statistics(&self) -> Result<Vec<CapabilityStats>> {
        let agents = self.repository.list().await?;
        let mut capability_counts: std::collections::HashMap<String, CapabilityStats> =
            std::collections::HashMap::new();

        for agent in agents {
            for capability in &agent.capabilities {
                let stats =
                    capability_counts
                        .entry(capability.clone())
                        .or_insert(CapabilityStats {
                            capability: capability.clone(),
                            total_agents: 0,
                            online_agents: 0,
                            available_agents: 0,
                            average_performance_score: 0.0,
                        });

                stats.total_agents += 1;
                if matches!(
                    agent.status,
                    vibe_ensemble_core::agent::AgentStatus::Online
                        | vibe_ensemble_core::agent::AgentStatus::Idle
                        | vibe_ensemble_core::agent::AgentStatus::Busy
                ) {
                    stats.online_agents += 1;
                }
                if agent.is_available() {
                    stats.available_agents += 1;
                }
                stats.average_performance_score += agent.performance_metrics.success_rate();
            }
        }

        // Calculate averages
        for stats in capability_counts.values_mut() {
            if stats.total_agents > 0 {
                stats.average_performance_score /= stats.total_agents as f64;
            }
        }

        let mut result: Vec<_> = capability_counts.into_values().collect();
        result.sort_by(|a, b| a.capability.cmp(&b.capability));
        Ok(result)
    }

    /// Assign a task to an agent and update its resource allocation
    pub async fn assign_task_to_agent(&self, agent_id: Uuid, task_id: String) -> Result<()> {
        info!("Assigning task {} to agent {}", task_id, agent_id);

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        // Check if agent can accept the task
        agent.assign_task().map_err(Error::Core)?;

        // Update the agent in the database
        self.repository
            .update_status(agent_id, &agent.status)
            .await?;
        // TODO: Update resource allocation in database when we add persistence for it

        info!(
            "Successfully assigned task {} to agent {}",
            task_id, agent_id
        );
        Ok(())
    }

    /// Complete a task for an agent and update performance metrics
    pub async fn complete_task_for_agent(
        &self,
        agent_id: Uuid,
        task_id: String,
        response_time_ms: f64,
        success: bool,
    ) -> Result<()> {
        info!(
            "Completing task {} for agent {} (success: {}, response_time: {}ms)",
            task_id, agent_id, success, response_time_ms
        );

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        // Record task completion
        agent
            .complete_task(response_time_ms, success)
            .map_err(Error::Core)?;

        // Update the agent in the database
        self.repository
            .update_status(agent_id, &agent.status)
            .await?;
        // TODO: Update performance metrics and resource allocation in database

        info!(
            "Successfully completed task {} for agent {}",
            task_id, agent_id
        );
        Ok(())
    }

    /// Update agent performance metrics
    pub async fn update_agent_performance(
        &self,
        agent_id: Uuid,
        cpu_percent: f32,
        memory_percent: f32,
    ) -> Result<()> {
        debug!("Updating performance metrics for agent {}", agent_id);

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        agent.update_performance(cpu_percent, memory_percent);

        // TODO: Persist performance metrics to database
        Ok(())
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

    /// Perform comprehensive health check on all agents
    pub async fn perform_health_check(&self) -> Result<HealthCheckResult> {
        info!("Performing comprehensive health check on all agents");

        let agents = self.repository.list().await?;
        let mut healthy_agents = Vec::new();
        let mut unhealthy_agents = Vec::new();
        let mut quarantined_agents = Vec::new();

        for agent in agents {
            if agent.health_info.is_quarantined() {
                quarantined_agents.push(agent.id);
            } else if matches!(
                agent.status,
                vibe_ensemble_core::agent::AgentStatus::Unhealthy { .. }
            ) {
                unhealthy_agents.push(agent.id);
            } else if agent.is_healthy(60) {
                healthy_agents.push(agent.id);
            } else {
                unhealthy_agents.push(agent.id);
                // Mark agent as unhealthy if not already
                if !matches!(
                    agent.status,
                    vibe_ensemble_core::agent::AgentStatus::Unhealthy { .. }
                ) {
                    let _ = self
                        .update_agent_status(
                            agent.id,
                            vibe_ensemble_core::agent::AgentStatus::Unhealthy {
                                reason: "Failed health check".to_string(),
                            },
                        )
                        .await;
                }
            }
        }

        let result = HealthCheckResult {
            healthy_count: healthy_agents.len(),
            unhealthy_count: unhealthy_agents.len(),
            quarantined_count: quarantined_agents.len(),
            healthy_agents,
            unhealthy_agents,
            quarantined_agents,
        };

        info!(
            "Health check complete: {} healthy, {} unhealthy, {} quarantined",
            result.healthy_count, result.unhealthy_count, result.quarantined_count
        );

        Ok(result)
    }

    /// Attempt recovery for unhealthy agents
    pub async fn attempt_agent_recovery(&self, agent_id: Uuid) -> Result<bool> {
        info!("Attempting recovery for agent {}", agent_id);

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        // Only attempt recovery for unhealthy agents
        if !matches!(
            agent.status,
            vibe_ensemble_core::agent::AgentStatus::Unhealthy { .. }
        ) {
            return Ok(false);
        }

        // Check if agent is still quarantined
        if agent.health_info.is_quarantined() {
            info!("Agent {} is still quarantined, recovery delayed", agent_id);
            return Ok(false);
        }

        // Attempt to transition back to online
        match agent.go_online() {
            Ok(()) => {
                self.repository
                    .update_status(agent_id, &agent.status)
                    .await?;
                info!("Successfully recovered agent {}", agent_id);
                Ok(true)
            }
            Err(e) => {
                warn!("Failed to recover agent {}: {}", agent_id, e);
                Ok(false)
            }
        }
    }

    /// Load balance tasks across available agents
    pub async fn get_load_balancer_recommendations(
        &self,
        task_count: usize,
    ) -> Result<LoadBalancerRecommendation> {
        info!(
            "Getting load balancer recommendations for {} tasks",
            task_count
        );

        let agents = self.list_online_agents().await?;
        let available_agents: Vec<_> = agents
            .into_iter()
            .filter(|agent| agent.is_available())
            .collect();

        if available_agents.is_empty() {
            return Ok(LoadBalancerRecommendation {
                total_capacity: 0,
                recommended_assignments: Vec::new(),
                overflow_tasks: task_count,
                load_distribution_score: 0.0,
            });
        }

        // Calculate total capacity
        let total_capacity: u32 = available_agents
            .iter()
            .map(|agent| {
                agent.resource_allocation.max_concurrent_tasks
                    - agent.resource_allocation.current_task_count
            })
            .sum();

        if total_capacity == 0 {
            return Ok(LoadBalancerRecommendation {
                total_capacity: 0,
                recommended_assignments: Vec::new(),
                overflow_tasks: task_count,
                load_distribution_score: 0.0,
            });
        }

        // Create weighted assignments based on load balancing scores
        let mut agent_scores: Vec<(Uuid, f64, u32)> = available_agents
            .iter()
            .map(|agent| {
                let available_slots = agent.resource_allocation.max_concurrent_tasks
                    - agent.resource_allocation.current_task_count;
                (
                    agent.id,
                    agent.calculate_load_balancing_score(),
                    available_slots,
                )
            })
            .collect();

        agent_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Distribute tasks based on scores and available capacity
        let mut assignments = Vec::new();
        let mut remaining_tasks = task_count;

        for (agent_id, score, available_slots) in agent_scores {
            if remaining_tasks == 0 {
                break;
            }

            let tasks_to_assign = std::cmp::min(remaining_tasks, available_slots as usize);
            if tasks_to_assign > 0 {
                assignments.push(TaskAssignment {
                    agent_id,
                    task_count: tasks_to_assign,
                    load_balancing_score: score,
                });
                remaining_tasks -= tasks_to_assign;
            }
        }

        // Calculate load distribution score (0.0 to 1.0, higher is better)
        let load_distribution_score = if available_agents.len() > 1 {
            let average_load: f32 = available_agents
                .iter()
                .map(|agent| agent.resource_allocation.load_factor)
                .sum::<f32>()
                / available_agents.len() as f32;

            let variance: f32 = available_agents
                .iter()
                .map(|agent| (agent.resource_allocation.load_factor - average_load).powi(2))
                .sum::<f32>()
                / available_agents.len() as f32;

            1.0 - variance.sqrt() as f64 // Lower variance = better distribution
        } else {
            1.0
        };

        Ok(LoadBalancerRecommendation {
            total_capacity: total_capacity as usize,
            recommended_assignments: assignments,
            overflow_tasks: remaining_tasks,
            load_distribution_score,
        })
    }

    /// Get system health overview
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let statistics = self.get_statistics().await?;
        let capability_stats = self.get_capability_statistics().await?;
        let health_check = self.perform_health_check().await?;
        let load_balance = self.get_load_balancer_recommendations(0).await?;

        Ok(SystemHealth {
            agent_statistics: statistics,
            capability_statistics: capability_stats,
            health_status: health_check,
            load_distribution_score: load_balance.load_distribution_score,
            total_system_capacity: load_balance.total_capacity,
        })
    }

    /// Create an agent pool with specific agents
    pub async fn create_agent_pool(
        &self,
        pool_name: String,
        agent_ids: Vec<Uuid>,
        pool_config: AgentPoolConfig,
    ) -> Result<AgentPool> {
        info!(
            "Creating agent pool '{}' with {} agents",
            pool_name,
            agent_ids.len()
        );

        // Verify all agents exist and are available
        let mut agents = Vec::new();
        for agent_id in &agent_ids {
            let agent = self
                .repository
                .find_by_id(*agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;
            agents.push(agent);
        }

        let pool = AgentPool {
            id: Uuid::new_v4(),
            name: pool_name,
            agent_ids,
            config: pool_config,
            created_at: chrono::Utc::now(),
            status: AgentPoolStatus::Active,
            statistics: AgentPoolStatistics::default(),
        };

        info!("Created agent pool '{}' with ID {}", pool.name, pool.id);
        Ok(pool)
    }

    /// Get agents that match pool requirements
    pub async fn find_agents_for_pool(
        &self,
        required_capabilities: &[(String, f32)],
        min_agents: usize,
        max_agents: usize,
        agent_type_filter: Option<vibe_ensemble_core::agent::AgentType>,
    ) -> Result<Vec<Agent>> {
        info!(
            "Finding agents for pool: min={}, max={}, required_capabilities={}",
            min_agents,
            max_agents,
            required_capabilities.len()
        );

        let mut candidates = self.list_online_agents().await?;

        // Filter by agent type if specified
        if let Some(agent_type) = agent_type_filter {
            candidates.retain(|agent| agent.agent_type == agent_type);
        }

        // Filter by capabilities and score
        let mut scored_candidates: Vec<(Agent, f64)> = candidates
            .into_iter()
            .filter_map(|agent| {
                let (matches, capability_score) =
                    agent.matches_capabilities_weighted(required_capabilities);
                if matches && agent.is_available() {
                    let load_balancing_score = agent.calculate_load_balancing_score();
                    let combined_score = capability_score as f64 * 0.7 + load_balancing_score * 0.3;
                    Some((agent, combined_score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        scored_candidates
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take up to max_agents
        let selected: Vec<Agent> = scored_candidates
            .into_iter()
            .take(max_agents)
            .map(|(agent, _)| agent)
            .collect();

        if selected.len() < min_agents {
            return Err(Error::ConstraintViolation(format!(
                "Could not find minimum {} agents for pool (found {})",
                min_agents,
                selected.len()
            )));
        }

        info!("Found {} agents for pool", selected.len());
        Ok(selected)
    }

    /// Get pool performance metrics
    pub async fn get_pool_performance(&self, pool: &AgentPool) -> Result<AgentPoolPerformance> {
        let mut total_tasks_completed = 0u64;
        let mut total_tasks_failed = 0u64;
        let mut total_response_time = 0.0f64;
        let mut available_agents = 0usize;
        let mut busy_agents = 0usize;
        let mut offline_agents = 0usize;

        for agent_id in &pool.agent_ids {
            if let Some(agent) = self.repository.find_by_id(*agent_id).await? {
                total_tasks_completed += agent.performance_metrics.tasks_completed;
                total_tasks_failed += agent.performance_metrics.tasks_failed;
                total_response_time += agent.performance_metrics.average_response_time_ms;

                match agent.status {
                    vibe_ensemble_core::agent::AgentStatus::Online
                    | vibe_ensemble_core::agent::AgentStatus::Idle => {
                        if agent.is_available() {
                            available_agents += 1;
                        }
                    }
                    vibe_ensemble_core::agent::AgentStatus::Busy => busy_agents += 1,
                    _ => offline_agents += 1,
                }
            } else {
                offline_agents += 1;
            }
        }

        let total_agents = pool.agent_ids.len();
        let average_response_time = if total_agents > 0 {
            total_response_time / total_agents as f64
        } else {
            0.0
        };

        let success_rate = if total_tasks_completed + total_tasks_failed > 0 {
            (total_tasks_completed as f64 / (total_tasks_completed + total_tasks_failed) as f64)
                * 100.0
        } else {
            100.0
        };

        Ok(AgentPoolPerformance {
            pool_id: pool.id,
            total_agents,
            available_agents,
            busy_agents,
            offline_agents,
            total_tasks_completed,
            total_tasks_failed,
            success_rate,
            average_response_time_ms: average_response_time,
            utilization_rate: if total_agents > 0 {
                (busy_agents as f64 / total_agents as f64) * 100.0
            } else {
                0.0
            },
        })
    }

    /// Rebalance agents in a pool based on current performance
    pub async fn rebalance_agent_pool(&self, pool: &mut AgentPool) -> Result<Vec<Uuid>> {
        info!("Rebalancing agent pool '{}'", pool.name);

        let _performance = self.get_pool_performance(pool).await?;
        let mut changes = Vec::new();

        // Remove offline or unhealthy agents
        let mut agents_to_remove = Vec::new();
        for agent_id in &pool.agent_ids {
            if let Some(agent) = self.repository.find_by_id(*agent_id).await? {
                if matches!(
                    agent.status,
                    vibe_ensemble_core::agent::AgentStatus::Offline
                        | vibe_ensemble_core::agent::AgentStatus::Unhealthy { .. }
                ) || agent.health_info.is_quarantined()
                {
                    agents_to_remove.push(*agent_id);
                }
            } else {
                agents_to_remove.push(*agent_id);
            }
        }

        for agent_id in &agents_to_remove {
            pool.agent_ids.retain(|id| id != agent_id);
            changes.push(*agent_id);
            info!(
                "Removed unhealthy agent {} from pool '{}'",
                agent_id, pool.name
            );
        }

        // If pool is below minimum size, try to add more agents
        if pool.agent_ids.len() < pool.config.min_agents {
            let needed = pool.config.min_agents - pool.agent_ids.len();
            let exclude_current: Vec<Uuid> = pool.agent_ids.clone();

            let candidates = self
                .find_agents_for_parallel_task(
                    &pool.config.required_capabilities,
                    needed,
                    Some(&exclude_current),
                )
                .await?;

            for candidate in candidates {
                if pool.agent_ids.len() < pool.config.max_agents {
                    pool.agent_ids.push(candidate.id);
                    changes.push(candidate.id);
                    info!("Added agent {} to pool '{}'", candidate.id, pool.name);
                }
            }
        }

        info!(
            "Rebalanced pool '{}': {} changes made",
            pool.name,
            changes.len()
        );
        Ok(changes)
    }

    /// Assign a system prompt to an agent
    pub async fn assign_system_prompt_to_agent(
        &self,
        agent_id: Uuid,
        prompt_id: Uuid,
        prompt_version: String,
        context_data: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        info!(
            "Assigning system prompt {} to agent {}",
            prompt_id, agent_id
        );

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        agent
            .assign_system_prompt(prompt_id, prompt_version, context_data)
            .map_err(Error::Core)?;

        // TODO: Update agent in database with system prompt assignment
        info!(
            "Successfully assigned system prompt {} to agent {}",
            prompt_id, agent_id
        );
        Ok(())
    }

    /// Clear system prompt assignment from an agent
    pub async fn clear_agent_system_prompt(&self, agent_id: Uuid) -> Result<()> {
        info!("Clearing system prompt from agent {}", agent_id);

        let mut agent =
            self.repository
                .find_by_id(agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;

        agent.clear_system_prompt();

        // TODO: Update agent in database
        info!("Successfully cleared system prompt from agent {}", agent_id);
        Ok(())
    }

    /// Assign system prompts based on agent type and capabilities
    pub async fn auto_assign_system_prompts(&self) -> Result<Vec<(Uuid, Option<Uuid>)>> {
        info!("Auto-assigning system prompts to agents");

        let agents = self.repository.list().await?;
        let mut assignments = Vec::new();

        for agent in agents {
            if !agent.has_system_prompt() && agent.is_available() {
                // Simple logic for auto-assignment based on agent type
                let prompt_assignment = match agent.agent_type {
                    vibe_ensemble_core::agent::AgentType::Coordinator => {
                        // Assign coordinator-specific prompt
                        // TODO: Get actual prompt ID from prompt service
                        Some(Uuid::new_v4()) // Placeholder
                    }
                    vibe_ensemble_core::agent::AgentType::Worker => {
                        // Assign worker-specific prompt based on capabilities
                        // TODO: Get actual prompt IDs from prompt service
                        // For now, using different static UUIDs for different capabilities
                        if agent.has_capability("code-review") {
                            Some(Uuid::parse_str("12345678-1234-5678-9abc-def012345678").unwrap())
                        // Code review prompt
                        } else if agent.has_capability("testing") {
                            Some(Uuid::parse_str("87654321-4321-8765-cba9-fed987654321").unwrap())
                        // Testing prompt
                        } else {
                            Some(Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap())
                            // General worker prompt
                        }
                    }
                };

                assignments.push((agent.id, prompt_assignment));

                if let Some(prompt_id) = prompt_assignment {
                    let context = std::collections::HashMap::from([
                        ("agent_type".to_string(), format!("{:?}", agent.agent_type)),
                        ("capabilities".to_string(), agent.capabilities.join(",")),
                    ]);

                    let _ = self
                        .assign_system_prompt_to_agent(
                            agent.id,
                            prompt_id,
                            "1.0".to_string(),
                            context,
                        )
                        .await;
                }
            }
        }

        info!("Auto-assigned {} system prompts", assignments.len());
        Ok(assignments)
    }

    /// Get agents by system prompt assignment
    pub async fn get_agents_with_prompt(
        &self,
        prompt_id: Uuid,
    ) -> Result<Vec<vibe_ensemble_core::agent::Agent>> {
        let agents = self.repository.list().await?;
        let filtered: Vec<_> = agents
            .into_iter()
            .filter(|agent| agent.get_system_prompt_id() == Some(prompt_id))
            .collect();

        Ok(filtered)
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

/// Statistics for a specific capability across agents
#[derive(Debug, Clone)]
pub struct CapabilityStats {
    pub capability: String,
    pub total_agents: i64,
    pub online_agents: i64,
    pub available_agents: i64,
    pub average_performance_score: f64,
}

/// Result of a comprehensive health check
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy_count: usize,
    pub unhealthy_count: usize,
    pub quarantined_count: usize,
    pub healthy_agents: Vec<Uuid>,
    pub unhealthy_agents: Vec<Uuid>,
    pub quarantined_agents: Vec<Uuid>,
}

/// Task assignment recommendation for load balancing
#[derive(Debug, Clone)]
pub struct TaskAssignment {
    pub agent_id: Uuid,
    pub task_count: usize,
    pub load_balancing_score: f64,
}

/// Load balancer recommendation for task distribution
#[derive(Debug, Clone)]
pub struct LoadBalancerRecommendation {
    pub total_capacity: usize,
    pub recommended_assignments: Vec<TaskAssignment>,
    pub overflow_tasks: usize,
    pub load_distribution_score: f64,
}

/// Overall system health information
#[derive(Debug, Clone)]
pub struct SystemHealth {
    pub agent_statistics: AgentStatistics,
    pub capability_statistics: Vec<CapabilityStats>,
    pub health_status: HealthCheckResult,
    pub load_distribution_score: f64,
    pub total_system_capacity: usize,
}

/// Configuration for an agent pool
#[derive(Debug, Clone)]
pub struct AgentPoolConfig {
    pub min_agents: usize,
    pub max_agents: usize,
    pub required_capabilities: Vec<(String, f32)>,
    pub auto_scaling: bool,
    pub health_check_interval_seconds: u64,
}

/// Status of an agent pool
#[derive(Debug, Clone, PartialEq)]
pub enum AgentPoolStatus {
    Active,
    Scaling,
    Maintenance,
    Inactive,
}

/// Statistics for an agent pool
#[derive(Debug, Clone)]
pub struct AgentPoolStatistics {
    pub total_tasks_processed: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub average_response_time_ms: f64,
    pub last_scaling_event: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for AgentPoolStatistics {
    fn default() -> Self {
        Self {
            total_tasks_processed: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            average_response_time_ms: 0.0,
            last_scaling_event: None,
        }
    }
}

/// Agent pool for managing groups of agents
#[derive(Debug, Clone)]
pub struct AgentPool {
    pub id: Uuid,
    pub name: String,
    pub agent_ids: Vec<Uuid>,
    pub config: AgentPoolConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: AgentPoolStatus,
    pub statistics: AgentPoolStatistics,
}

/// Performance metrics for an agent pool
#[derive(Debug, Clone)]
pub struct AgentPoolPerformance {
    pub pool_id: Uuid,
    pub total_agents: usize,
    pub available_agents: usize,
    pub busy_agents: usize,
    pub offline_agents: usize,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub success_rate: f64,
    pub average_response_time_ms: f64,
    pub utilization_rate: f64,
}

#[cfg(test)]
mod tests {
    include!("agent_tests.rs");
}
