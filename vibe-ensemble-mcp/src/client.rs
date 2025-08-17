//! MCP client implementation

use crate::{protocol::*, transport::Transport, Error, Result};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use uuid::Uuid;

/// MCP client for connecting to MCP servers
pub struct McpClient {
    transport: Box<dyn Transport>,
    client_info: ClientInfo,
    capabilities: ClientCapabilities,
    session_id: Option<Uuid>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new(
        transport: Box<dyn Transport>,
        client_info: ClientInfo,
        capabilities: ClientCapabilities,
    ) -> Self {
        Self {
            transport,
            client_info,
            capabilities,
            session_id: None,
        }
    }

    /// Initialize connection with the server
    pub async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: MCP_VERSION.to_string(),
            client_info: self.client_info.clone(),
            capabilities: self.capabilities.clone(),
        };

        let message = McpMessage::new_request(
            methods::INITIALIZE.to_string(),
            serde_json::to_value(params)?,
        );

        self.send_message(message.clone()).await?;
        
        let response = self.receive_message().await?;
        
        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("Initialization failed: {}", error.message),
            });
        }

        self.session_id = Some(message.id);
        info!("MCP client initialized successfully");
        Ok(())
    }

    /// Send a ping message
    pub async fn ping(&mut self) -> Result<()> {
        let message = McpMessage::new_request(
            methods::PING.to_string(),
            serde_json::Value::Null,
        );

        self.send_message(message).await?;
        let response = self.receive_message().await?;

        if response.error.is_some() {
            return Err(Error::Protocol {
                message: "Ping failed".to_string(),
            });
        }

        debug!("Ping successful");
        Ok(())
    }

    /// Register as an agent
    pub async fn register_agent(&mut self, agent_info: serde_json::Value) -> Result<Uuid> {
        let message = McpMessage::new_request(
            methods::AGENT_REGISTER.to_string(),
            agent_info,
        );

        self.send_message(message).await?;
        let response = self.receive_message().await?;

        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("Agent registration failed: {}", error.message),
            });
        }

        let result = response.result.ok_or_else(|| Error::Protocol {
            message: "No result in registration response".to_string(),
        })?;

        let agent_id: Uuid = serde_json::from_value(
            result.get("agent_id")
                .ok_or_else(|| Error::Protocol {
                    message: "No agent_id in response".to_string(),
                })?
                .clone(),
        )?;

        info!("Agent registered with ID: {}", agent_id);
        Ok(agent_id)
    }

    /// Send a message to the server
    async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        let json = serde_json::to_string(&message)?;
        self.transport.send(&json).await?;
        debug!("Sent message: {}", message.method);
        Ok(())
    }

    /// Receive a message from the server
    async fn receive_message(&mut self) -> Result<McpMessage> {
        let json = self.transport.receive().await?;
        let message: McpMessage = serde_json::from_str(&json)?;
        debug!("Received message response");
        Ok(message)
    }

    /// Close the client connection
    pub async fn close(&mut self) -> Result<()> {
        self.transport.close().await?;
        info!("MCP client connection closed");
        Ok(())
    }
}