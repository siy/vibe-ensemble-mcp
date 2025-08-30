//! Mock coordinator and worker agents for integration testing
//!
//! This module provides realistic mock agents that interact with the MCP server
//! using actual MCP tool calls, simulating real coordinator-worker workflows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use vibe_ensemble_mcp::{
    protocol::{
        AgentRegisterParams, IssueAssignParams, IssueCreateParams, IssueUpdateParams,
        JsonRpcRequest,
    },
    server::McpServer,
};

/// Mock coordinator agent that creates and assigns tickets via MCP tools
#[derive(Debug)]
pub struct MockCoordinator {
    /// Agent ID
    id: Uuid,
    /// Agent name
    name: String,
    /// Agent capabilities
    capabilities: Vec<String>,
    // transport intentionally omitted; use server.handle_message directly
    /// Created tickets tracking
    created_tickets: Arc<RwLock<Vec<Uuid>>>,
}

impl MockCoordinator {
    /// Creates a new mock coordinator
    pub async fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            capabilities: vec![
                "coordination".to_string(),
                "ticket_management".to_string(),
                "task_assignment".to_string(),
            ],
            created_tickets: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Gets the coordinator ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Gets the coordinator name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Registers the coordinator with the MCP server
    pub async fn register_with_server(
        &mut self,
        server: &McpServer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create registration request
        let register_params = AgentRegisterParams {
            name: self.name.clone(),
            agent_type: "coordinator".to_string(),
            capabilities: self.capabilities.clone(),
            connection_metadata: serde_json::json!({
                "test_mode": true,
                "coordinator_id": self.id
            }),
        };

        let request = JsonRpcRequest::new(
            "vibe/agent/register",
            Some(serde_json::to_value(register_params)?),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request)?)
            .await?
            .ok_or("No response from server")?;

        let response_value: serde_json::Value = serde_json::from_str(&response)?;
        if response_value.get("error").is_some() {
            return Err(format!("Agent registration failed: {}", response).into());
        }

        Ok(())
    }

    /// Creates tickets via MCP tools
    pub async fn create_tickets(
        &self,
        server: &McpServer,
        ticket_definitions: Vec<super::TicketDefinition>,
    ) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
        let mut ticket_ids = Vec::new();

        for ticket_def in ticket_definitions {
            let create_params = IssueCreateParams {
                title: ticket_def.title,
                description: ticket_def.description,
                priority: Some(ticket_def.priority),
                issue_type: Some("task".to_string()),
                project_id: Some("integration-test".to_string()),
                created_by_agent_id: self.id.to_string(),
                labels: None,
                assignee: None,
            };

            let request = JsonRpcRequest::new(
                "vibe/issue/create",
                Some(serde_json::to_value(create_params)?),
            );

            let response = server
                .handle_message(&serde_json::to_string(&request)?)
                .await?
                .ok_or("No response from server")?;

            let response_value: serde_json::Value = serde_json::from_str(&response)?;

            if let Some(error) = response_value.get("error") {
                return Err(format!("Ticket creation failed: {}", error).into());
            }

            if let Some(result) = response_value.get("result") {
                if let Some(issue_id) = result.get("issueId") {
                    let id = Uuid::parse_str(issue_id.as_str().unwrap())?;
                    ticket_ids.push(id);

                    // Track created tickets
                    self.created_tickets.write().await.push(id);
                }
            }
        }

        Ok(ticket_ids)
    }

    /// Assigns a ticket to a worker via MCP tools
    pub async fn assign_ticket(
        &self,
        server: &McpServer,
        ticket_id: Uuid,
        worker_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let assign_params = IssueAssignParams {
            issue_id: ticket_id.to_string(),
            assignee_agent_id: worker_id.to_string(),
            assigned_by_agent_id: self.id.to_string(),
            reason: Some("Automated assignment for integration test".to_string()),
        };

        let request = JsonRpcRequest::new(
            "vibe/issue/assign",
            Some(serde_json::to_value(assign_params)?),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request)?)
            .await?
            .ok_or("No response from server")?;

        let response_value: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(error) = response_value.get("error") {
            return Err(format!("Ticket assignment failed: {}", error).into());
        }

        Ok(())
    }

    /// Gets list of created tickets
    pub async fn created_tickets(&self) -> Vec<Uuid> {
        self.created_tickets.read().await.clone()
    }
}

/// Mock worker agent that creates files and updates tickets via MCP tools
#[derive(Debug)]
pub struct MockWorker {
    /// Agent ID
    id: Uuid,
    /// Agent name
    name: String,
    /// Agent capabilities
    capabilities: Vec<String>,
    // transport intentionally omitted; use server.handle_message directly
    /// Files created by this worker
    created_files: Arc<RwLock<Vec<PathBuf>>>,
}

impl MockWorker {
    /// Creates a new mock worker
    pub async fn new(
        name: &str,
        capabilities: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            capabilities,
            created_files: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Gets the worker ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Gets the worker name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Registers the worker with the MCP server
    pub async fn register_with_server(
        &mut self,
        server: &McpServer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create registration request
        let register_params = AgentRegisterParams {
            name: self.name.clone(),
            agent_type: "worker".to_string(),
            capabilities: self.capabilities.clone(),
            connection_metadata: serde_json::json!({
                "test_mode": true,
                "worker_id": self.id
            }),
        };

        let request = JsonRpcRequest::new(
            "vibe/agent/register",
            Some(serde_json::to_value(register_params)?),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request)?)
            .await?
            .ok_or("No response from server")?;

        let response_value: serde_json::Value = serde_json::from_str(&response)?;
        if response_value.get("error").is_some() {
            return Err(format!("Worker registration failed: {}", response).into());
        }

        Ok(())
    }

    /// Creates files in a specified worktree (simulates actual work)
    pub async fn create_files_in_worktree(
        &self,
        worktree_path: PathBuf,
        files: Vec<super::ExpectedFile>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for file_spec in files {
            let file_path = worktree_path.join(&file_spec.path);

            // Create parent directories if needed
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            // Create the file with expected content
            tokio::fs::write(&file_path, &file_spec.expected_content).await?;

            // Track created files
            self.created_files.write().await.push(file_path);
        }

        // Simulate some work time
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(())
    }

    /// Updates ticket status via MCP tools
    pub async fn update_ticket_status(
        &self,
        server: &McpServer,
        ticket_id: Uuid,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let update_params = IssueUpdateParams {
            issue_id: ticket_id.to_string(),
            status: Some(status.to_string()),
            comment: Some(format!("Work completed by worker {}", self.name)),
            updated_by_agent_id: self.id.to_string(),
            priority: None,
        };

        let request = JsonRpcRequest::new(
            "vibe/issue/update",
            Some(serde_json::to_value(update_params)?),
        );

        let response = server
            .handle_message(&serde_json::to_string(&request)?)
            .await?
            .ok_or("No response from server")?;

        let response_value: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(error) = response_value.get("error") {
            return Err(format!("Ticket update failed: {}", error).into());
        }

        Ok(())
    }

    /// Gets list of files created by this worker
    pub async fn created_files(&self) -> Vec<PathBuf> {
        self.created_files.read().await.clone()
    }

    /// Simulates worker going offline (for failure testing)
    pub async fn go_offline(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would disconnect the transport
        // For testing, we just simulate the offline state
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(())
    }

    /// Simulates worker coming back online (for recovery testing)
    pub async fn go_online(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would reconnect the transport
        // For testing, we just simulate coming back online
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(())
    }
}

/// Factory for creating mock agents with predefined configurations
pub struct MockAgentFactory;

impl MockAgentFactory {
    /// Creates a coordinator with standard configuration
    pub async fn coordinator(name: &str) -> Result<MockCoordinator, Box<dyn std::error::Error>> {
        MockCoordinator::new(name).await
    }

    /// Creates a backend development worker
    pub async fn backend_worker(name: &str) -> Result<MockWorker, Box<dyn std::error::Error>> {
        MockWorker::new(
            name,
            vec![
                "rust".to_string(),
                "backend".to_string(),
                "api".to_string(),
                "database".to_string(),
            ],
        )
        .await
    }

    /// Creates a frontend development worker
    pub async fn frontend_worker(name: &str) -> Result<MockWorker, Box<dyn std::error::Error>> {
        MockWorker::new(
            name,
            vec![
                "javascript".to_string(),
                "typescript".to_string(),
                "react".to_string(),
                "frontend".to_string(),
            ],
        )
        .await
    }

    /// Creates a QA/testing worker
    pub async fn qa_worker(name: &str) -> Result<MockWorker, Box<dyn std::error::Error>> {
        MockWorker::new(
            name,
            vec![
                "testing".to_string(),
                "quality_assurance".to_string(),
                "automation".to_string(),
            ],
        )
        .await
    }

    /// Creates a DevOps worker
    pub async fn devops_worker(name: &str) -> Result<MockWorker, Box<dyn std::error::Error>> {
        MockWorker::new(
            name,
            vec![
                "devops".to_string(),
                "deployment".to_string(),
                "infrastructure".to_string(),
                "monitoring".to_string(),
            ],
        )
        .await
    }

    /// Creates a generic worker with custom capabilities
    pub async fn custom_worker(
        name: &str,
        capabilities: Vec<String>,
    ) -> Result<MockWorker, Box<dyn std::error::Error>> {
        MockWorker::new(name, capabilities).await
    }
}

/// Utility for simulating realistic agent interactions
pub struct AgentInteractionSimulator {
    /// Delay between agent actions to simulate realistic timing
    action_delay: tokio::time::Duration,
    /// Whether to introduce random delays
    randomize_delays: bool,
}

impl AgentInteractionSimulator {
    /// Creates a new simulator with default settings
    pub fn new() -> Self {
        Self {
            action_delay: tokio::time::Duration::from_millis(100),
            randomize_delays: true,
        }
    }

    /// Sets a fixed delay between actions
    pub fn with_fixed_delay(mut self, delay: tokio::time::Duration) -> Self {
        self.action_delay = delay;
        self.randomize_delays = false;
        self
    }

    /// Enables random delays for more realistic simulation
    pub fn with_random_delays(mut self) -> Self {
        self.randomize_delays = true;
        self
    }

    /// Simulates delay before next action
    pub async fn simulate_work_delay(&self) {
        if self.randomize_delays {
            let jitter = rand::random::<f64>() * 0.5 + 0.5; // 0.5x to 1.0x multiplier
            let delay = self.action_delay.mul_f64(jitter);
            tokio::time::sleep(delay).await;
        } else {
            tokio::time::sleep(self.action_delay).await;
        }
    }

    /// Simulates coordinator thinking time before assigning work
    pub async fn simulate_coordination_planning(&self) {
        let planning_time = self.action_delay * 2;
        tokio::time::sleep(planning_time).await;
    }

    /// Simulates worker analysis time before starting work
    pub async fn simulate_worker_analysis(&self) {
        let analysis_time = if self.randomize_delays {
            let factor = rand::random::<f64>() * 1.5 + 0.5; // 0.5x to 2.0x
            self.action_delay.mul_f64(factor)
        } else {
            self.action_delay
        };
        tokio::time::sleep(analysis_time).await;
    }
}

impl Default for AgentInteractionSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_coordinator_creation() {
        let coordinator = MockCoordinator::new("test-coordinator")
            .await
            .expect("Failed to create coordinator");

        assert_eq!(coordinator.name(), "test-coordinator");
        assert!(!coordinator.id().to_string().is_empty());
        assert!(coordinator
            .capabilities
            .contains(&"coordination".to_string()));
    }

    #[tokio::test]
    async fn test_mock_worker_creation() {
        let worker = MockWorker::new("test-worker", vec!["rust".to_string()])
            .await
            .expect("Failed to create worker");

        assert_eq!(worker.name(), "test-worker");
        assert!(!worker.id().to_string().is_empty());
        assert!(worker.capabilities.contains(&"rust".to_string()));
    }

    #[tokio::test]
    async fn test_agent_factory() {
        let backend_worker = MockAgentFactory::backend_worker("backend-dev")
            .await
            .expect("Failed to create backend worker");

        assert!(backend_worker.capabilities.contains(&"rust".to_string()));
        assert!(backend_worker.capabilities.contains(&"backend".to_string()));

        let frontend_worker = MockAgentFactory::frontend_worker("frontend-dev")
            .await
            .expect("Failed to create frontend worker");

        assert!(frontend_worker.capabilities.contains(&"react".to_string()));
        assert!(frontend_worker
            .capabilities
            .contains(&"frontend".to_string()));
    }

    #[tokio::test]
    async fn test_interaction_simulator() {
        let simulator = AgentInteractionSimulator::new()
            .with_fixed_delay(tokio::time::Duration::from_millis(10));

        let start = std::time::Instant::now();
        simulator.simulate_work_delay().await;
        let elapsed = start.elapsed();

        // Should be approximately 10ms (allowing for some variance)
        assert!(elapsed >= tokio::time::Duration::from_millis(8));
        assert!(elapsed <= tokio::time::Duration::from_millis(20));
    }
}
