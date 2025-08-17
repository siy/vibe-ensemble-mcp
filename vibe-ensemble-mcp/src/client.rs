//! MCP client implementation
//!
//! This module provides an MCP client that can connect to MCP servers
//! using JSON-RPC 2.0 protocol over various transports.

use crate::{protocol::*, transport::Transport, Error, Result};
use tracing::{debug, info};

/// MCP client for connecting to MCP servers
pub struct McpClient {
    transport: Box<dyn Transport>,
    client_info: ClientInfo,
    capabilities: ClientCapabilities,
    session_id: Option<String>,
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
    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: MCP_VERSION.to_string(),
            client_info: self.client_info.clone(),
            capabilities: self.capabilities.clone(),
        };

        let request = JsonRpcRequest::new(
            methods::INITIALIZE,
            Some(serde_json::to_value(params)?),
        );

        let response = self.send_request(request).await?;
        
        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("Initialization failed: {}", error.message),
            });
        }

        let result: InitializeResult = serde_json::from_value(
            response.result.ok_or_else(|| Error::Protocol {
                message: "No result in initialization response".to_string(),
            })?
        )?;

        // Store session ID for future reference
        self.session_id = Some(match &response.id {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => "unknown".to_string(),
        });
        
        info!(
            "MCP client initialized successfully with server: {} v{}",
            result.server_info.name, result.server_info.version
        );
        
        Ok(result)
    }

    /// Send a ping message to check connection health
    pub async fn ping(&mut self) -> Result<serde_json::Value> {
        let request = JsonRpcRequest::new(methods::PING, None);
        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("Ping failed: {}", error.message),
            });
        }

        let result = response.result.ok_or_else(|| Error::Protocol {
            message: "No result in ping response".to_string(),
        })?;

        debug!("Ping successful");
        Ok(result)
    }

    /// Register as an agent with the Vibe Ensemble server
    pub async fn register_agent(&mut self, params: AgentRegisterParams) -> Result<AgentRegisterResult> {
        let request = JsonRpcRequest::new(
            methods::AGENT_REGISTER,
            Some(serde_json::to_value(params)?),
        );

        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("Agent registration failed: {}", error.message),
            });
        }

        let result: AgentRegisterResult = serde_json::from_value(
            response.result.ok_or_else(|| Error::Protocol {
                message: "No result in registration response".to_string(),
            })?
        )?;

        info!("Agent registered with ID: {}", result.agent_id);
        Ok(result)
    }

    /// List available tools from the server
    pub async fn list_tools(&mut self) -> Result<serde_json::Value> {
        let request = JsonRpcRequest::new(methods::LIST_TOOLS, None);
        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("List tools failed: {}", error.message),
            });
        }

        response.result.ok_or_else(|| Error::Protocol {
            message: "No result in list tools response".to_string(),
        })
    }

    /// List available resources from the server
    pub async fn list_resources(&mut self) -> Result<serde_json::Value> {
        let request = JsonRpcRequest::new(methods::LIST_RESOURCES, None);
        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(Error::Protocol {
                message: format!("List resources failed: {}", error.message),
            });
        }

        response.result.ok_or_else(|| Error::Protocol {
            message: "No result in list resources response".to_string(),
        })
    }

    /// Send a JSON-RPC request and receive response
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let request_json = serde_json::to_string(&request)?;
        self.transport.send(&request_json).await?;
        debug!("Sent request: {}", request.method);

        let response_json = self.transport.receive().await?;
        let response: JsonRpcResponse = serde_json::from_str(&response_json)?;
        debug!("Received response for request ID: {:?}", response.id);
        
        Ok(response)
    }

    /// Send a notification (no response expected)
    pub async fn send_notification(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = JsonRpcNotification::new(method, params);
        let notification_json = serde_json::to_string(&notification)?;
        self.transport.send(&notification_json).await?;
        debug!("Sent notification: {}", notification.method);
        Ok(())
    }

    /// Get the current session ID
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Get client info
    pub fn client_info(&self) -> &ClientInfo {
        &self.client_info
    }

    /// Get client capabilities
    pub fn capabilities(&self) -> &ClientCapabilities {
        &self.capabilities
    }

    /// Close the client connection
    pub async fn close(&mut self) -> Result<()> {
        self.transport.close().await?;
        info!("MCP client connection closed");
        Ok(())
    }
}