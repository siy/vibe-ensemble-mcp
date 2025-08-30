//! Agent testing utilities for vibe-ensemble-mcp
//!
//! Provides utilities for testing agent interactions and behaviors.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus, ConnectionMetadata},
    message::{Message, MessageType},
};

/// Mock agent for testing that simulates real agent behavior
#[derive(Clone)]
pub struct MockAgent {
    inner: Agent,
    message_inbox: Arc<Mutex<Vec<Message>>>,
    message_outbox: Arc<Mutex<Vec<Message>>>,
    is_connected: Arc<Mutex<bool>>,
}

impl MockAgent {
    /// Creates a new mock agent with basic capabilities
    pub fn new(name: &str) -> Self {
        let agent = Agent::builder()
            .name(name)
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8080,
                protocol: "test".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: format!("test-{}", Uuid::new_v4()),
            })
            .capabilities(vec!["testing".to_string(), "mock".to_string()])
            .build()
            .expect("Failed to create mock agent");

        Self {
            inner: agent,
            message_inbox: Arc::new(Mutex::new(Vec::new())),
            message_outbox: Arc::new(Mutex::new(Vec::new())),
            is_connected: Arc::new(Mutex::new(true)),
        }
    }

    /// Creates a specialized mock agent with specific capabilities
    pub fn with_capabilities(name: &str, capabilities: Vec<String>) -> Self {
        let agent = Agent::builder()
            .name(name)
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8080,
                protocol: "test".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: format!("test-{}", Uuid::new_v4()),
            })
            .capabilities(capabilities)
            .build()
            .expect("Failed to create mock agent");

        Self {
            inner: agent,
            message_inbox: Arc::new(Mutex::new(Vec::new())),
            message_outbox: Arc::new(Mutex::new(Vec::new())),
            is_connected: Arc::new(Mutex::new(true)),
        }
    }

    /// Gets the underlying agent
    pub fn agent(&self) -> &Agent {
        &self.inner
    }

    /// Gets the agent ID
    pub fn id(&self) -> Uuid {
        self.inner.id()
    }

    /// Gets the agent name
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// Simulates sending a message to another agent
    pub async fn send_message_to(&self, recipient: &MockAgent, content: &str) {
        let message = Message::direct(self.id(), recipient.id(), content)
            .expect("Failed to create direct message");

        // Add to sender's outbox
        self.message_outbox.lock().await.push(message.clone());

        // Add to recipient's inbox
        recipient.message_inbox.lock().await.push(message);
    }

    /// Simulates broadcasting a message
    pub async fn broadcast_message(&self, content: &str) {
        let message =
            Message::broadcast(self.id(), content).expect("Failed to create broadcast message");

        self.message_outbox.lock().await.push(message);
    }

    /// Gets all messages received by this agent
    pub async fn received_messages(&self) -> Vec<Message> {
        self.message_inbox.lock().await.clone()
    }

    /// Gets all messages sent by this agent
    pub async fn sent_messages(&self) -> Vec<Message> {
        self.message_outbox.lock().await.clone()
    }

    /// Simulates agent going offline
    pub async fn go_offline(&self) {
        *self.is_connected.lock().await = false;
    }

    /// Simulates agent coming back online
    pub async fn go_online(&self) {
        *self.is_connected.lock().await = true;
    }

    /// Checks if agent is currently connected
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.lock().await
    }

    /// Clears all messages (for test cleanup)
    pub async fn clear_messages(&self) {
        self.message_inbox.lock().await.clear();
        self.message_outbox.lock().await.clear();
    }

    /// Simulates processing a message and generating a response
    pub async fn process_message(&self, message: &Message) -> Option<Message> {
        // Simple message processing logic for testing
        if message.content().contains("ping") {
            Some(
                Message::direct(self.id(), message.sender_id(), "pong")
                    .expect("Failed to create pong message"),
            )
        } else if message.content().contains("status") {
            Some(
                Message::direct(
                    self.id(),
                    message.sender_id(),
                    &format!(
                        "Agent {} is active with {} capabilities",
                        self.name(),
                        self.inner.capabilities().len()
                    ),
                )
                .expect("Failed to create status message"),
            )
        } else {
            None
        }
    }
}

/// Agent network simulator for testing multi-agent scenarios
pub struct AgentNetwork {
    agents: HashMap<Uuid, MockAgent>,
    message_history: Arc<Mutex<Vec<Message>>>,
}

impl AgentNetwork {
    /// Creates a new empty agent network
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            message_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Adds an agent to the network
    pub fn add_agent(&mut self, agent: MockAgent) {
        self.agents.insert(agent.id(), agent);
    }

    /// Creates and adds multiple agents with specified capabilities
    pub fn add_agents_with_capabilities(&mut self, configs: Vec<(&str, Vec<String>)>) {
        for (name, capabilities) in configs {
            let agent = MockAgent::with_capabilities(name, capabilities);
            self.add_agent(agent);
        }
    }

    /// Gets an agent by ID
    pub fn get_agent(&self, id: &Uuid) -> Option<&MockAgent> {
        self.agents.get(id)
    }

    /// Gets an agent by name
    pub fn get_agent_by_name(&self, name: &str) -> Option<&MockAgent> {
        self.agents.values().find(|agent| agent.name() == name)
    }

    /// Gets all agents in the network
    pub fn all_agents(&self) -> Vec<&MockAgent> {
        self.agents.values().collect()
    }

    /// Simulates message delivery between agents in the network
    pub async fn deliver_message(
        &self,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        content: &str,
    ) {
        let message = if let Some(recipient) = recipient_id {
            Message::direct(sender_id, recipient, content)
        } else {
            Message::broadcast(sender_id, content)
        }
        .expect("Failed to create message");

        // Add to message history
        self.message_history.lock().await.push(message.clone());

        // Deliver to recipients
        if let Some(recipient_id) = recipient_id {
            // Direct message
            if let Some(recipient) = self.agents.get(&recipient_id) {
                recipient.message_inbox.lock().await.push(message);
            }
        } else {
            // Broadcast message - deliver to all agents except sender
            for (id, agent) in &self.agents {
                if *id != sender_id {
                    agent.message_inbox.lock().await.push(message.clone());
                }
            }
        }
    }

    /// Simulates a conversation between two agents
    pub async fn simulate_conversation(
        &self,
        agent1_id: Uuid,
        agent2_id: Uuid,
        message_count: usize,
    ) {
        for i in 0..message_count {
            let (sender_id, recipient_id) = if i % 2 == 0 {
                (agent1_id, agent2_id)
            } else {
                (agent2_id, agent1_id)
            };

            let content = format!("Message {} in conversation", i + 1);
            self.deliver_message(sender_id, Some(recipient_id), &content)
                .await;
        }
    }

    /// Gets the complete message history for the network
    pub async fn message_history(&self) -> Vec<Message> {
        self.message_history.lock().await.clone()
    }

    /// Simulates network partition where some agents lose connectivity
    pub async fn simulate_network_partition(&self, offline_agents: &[Uuid]) {
        for id in offline_agents {
            if let Some(agent) = self.agents.get(id) {
                agent.go_offline().await;
            }
        }
    }

    /// Restores connectivity for all agents
    pub async fn restore_network(&self) {
        for agent in self.agents.values() {
            agent.go_online().await;
        }
    }

    /// Gets network statistics
    pub async fn network_stats(&self) -> NetworkStats {
        let total_agents = self.agents.len();
        let active_agents = {
            let mut count = 0;
            for agent in self.agents.values() {
                if agent.is_connected().await {
                    count += 1;
                }
            }
            count
        };
        let total_messages = self.message_history.lock().await.len();

        NetworkStats {
            total_agents,
            active_agents,
            total_messages,
        }
    }
}

/// Network statistics for monitoring test scenarios
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_agents: usize,
    pub active_agents: usize,
    pub total_messages: usize,
}

/// Builder for creating complex agent network scenarios
pub struct AgentNetworkBuilder {
    network: AgentNetwork,
}

impl AgentNetworkBuilder {
    /// Creates a new network builder
    pub fn new() -> Self {
        Self {
            network: AgentNetwork::new(),
        }
    }

    /// Adds a coordinator agent
    pub fn with_coordinator(mut self, name: &str) -> Self {
        let agent = MockAgent::with_capabilities(
            name,
            vec![
                "coordination".to_string(),
                "planning".to_string(),
                "management".to_string(),
            ],
        );
        self.network.add_agent(agent);
        self
    }

    /// Adds worker agents with specified capabilities
    pub fn with_workers(
        mut self,
        count: usize,
        base_name: &str,
        capabilities: Vec<String>,
    ) -> Self {
        for i in 0..count {
            let name = format!("{}-{}", base_name, i);
            let agent = MockAgent::with_capabilities(&name, capabilities.clone());
            self.network.add_agent(agent);
        }
        self
    }

    /// Adds specialized agents
    pub fn with_specialist(mut self, name: &str, specialty: &str) -> Self {
        let capabilities = vec![
            specialty.to_string(),
            "analysis".to_string(),
            "support".to_string(),
        ];
        let agent = MockAgent::with_capabilities(name, capabilities);
        self.network.add_agent(agent);
        self
    }

    /// Builds the final agent network
    pub fn build(self) -> AgentNetwork {
        self.network
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_mock_agent_creation() {
        let agent = MockAgent::new("test-agent");
        assert_eq!(agent.name(), "test-agent");
        assert!(agent.is_connected().await);
        assert!(agent.received_messages().await.is_empty());
    }

    #[tokio::test]
    async fn test_agent_messaging() {
        let agent1 = MockAgent::new("agent1");
        let agent2 = MockAgent::new("agent2");

        agent1.send_message_to(&agent2, "Hello, agent2!").await;

        let received = agent2.received_messages().await;
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].content(), "Hello, agent2!");
        assert_eq!(received[0].sender_id(), agent1.id());

        let sent = agent1.sent_messages().await;
        assert_eq!(sent.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_network() {
        let mut network = AgentNetwork::new();
        let agent1 = MockAgent::new("agent1");
        let agent2 = MockAgent::new("agent2");
        let id1 = agent1.id();
        let id2 = agent2.id();

        network.add_agent(agent1);
        network.add_agent(agent2);

        network
            .deliver_message(id1, Some(id2), "Test message")
            .await;

        let stats = network.network_stats().await;
        assert_eq!(stats.total_agents, 2);
        assert_eq!(stats.active_agents, 2);
        assert_eq!(stats.total_messages, 1);
    }

    #[tokio::test]
    async fn test_network_builder() {
        let network = AgentNetworkBuilder::new()
            .with_coordinator("coordinator")
            .with_workers(3, "worker", vec!["coding".to_string()])
            .with_specialist("qa", "quality_assurance")
            .build();

        let stats = network.network_stats().await;
        assert_eq!(stats.total_agents, 5); // 1 coordinator + 3 workers + 1 specialist
        assert_eq!(stats.active_agents, 5);
    }

    #[tokio::test]
    async fn test_network_partition_simulation() {
        let mut network = AgentNetwork::new();
        let agent1 = MockAgent::new("agent1");
        let agent2 = MockAgent::new("agent2");
        let id1 = agent1.id();
        let _id2 = agent2.id();

        network.add_agent(agent1);
        network.add_agent(agent2);

        // Simulate network partition
        network.simulate_network_partition(&[id1]).await;

        let stats = network.network_stats().await;
        assert_eq!(stats.active_agents, 1); // Only agent2 is active

        // Restore network
        network.restore_network().await;
        let stats = network.network_stats().await;
        assert_eq!(stats.active_agents, 2); // Both agents active again
    }

    #[tokio::test]
    async fn test_conversation_simulation() {
        let mut network = AgentNetwork::new();
        let agent1 = MockAgent::new("agent1");
        let agent2 = MockAgent::new("agent2");
        let id1 = agent1.id();
        let id2 = agent2.id();

        network.add_agent(agent1);
        network.add_agent(agent2);

        network.simulate_conversation(id1, id2, 5).await;

        let history = network.message_history().await;
        assert_eq!(history.len(), 5);

        // Verify alternating conversation
        assert_eq!(history[0].sender_id(), id1);
        assert_eq!(history[1].sender_id(), id2);
        assert_eq!(history[2].sender_id(), id1);
    }
}
