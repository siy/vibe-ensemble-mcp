//! MCP server implementation

use crate::{protocol::*, transport::Transport, Error, Result};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

/// MCP server state
pub struct McpServer {
    clients: RwLock<HashMap<Uuid, ClientSession>>,
    capabilities: ServerCapabilities,
}

/// Client session information
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub id: Uuid,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

/// Server capabilities
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerCapabilities {
    pub agent_management: bool,
    pub issue_tracking: bool,
    pub messaging: bool,
    pub knowledge_management: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            agent_management: true,
            issue_tracking: true,
            messaging: true,
            knowledge_management: true,
        }
    }
}

impl McpServer {
    /// Create a new MCP server
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            capabilities: ServerCapabilities::default(),
        }
    }

    /// Handle an incoming MCP message
    pub async fn handle_message(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        debug!("Handling MCP message: {}", message.method);

        if message.is_request() {
            self.handle_request(message).await
        } else {
            // Handle responses/notifications
            Ok(None)
        }
    }

    /// Handle a request message
    async fn handle_request(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        match message.method.as_str() {
            methods::INITIALIZE => self.handle_initialize(message).await,
            methods::PING => self.handle_ping(message).await,
            methods::AGENT_REGISTER => self.handle_agent_register(message).await,
            _ => {
                error!("Unknown method: {}", message.method);
                Ok(Some(McpMessage::new_error(
                    message.id,
                    McpError {
                        code: -32601,
                        message: "Method not found".to_string(),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle initialization request
    async fn handle_initialize(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        let params: InitializeParams = serde_json::from_value(message.params)?;
        
        info!("Client initializing: {} v{}", 
              params.client_info.name, 
              params.client_info.version);

        let session = ClientSession {
            id: message.id,
            client_info: params.client_info,
            capabilities: params.capabilities,
            connected_at: chrono::Utc::now(),
        };

        self.clients.write().await.insert(message.id, session);

        let result = serde_json::json!({
            "protocol_version": MCP_VERSION,
            "server_info": {
                "name": "vibe-ensemble-mcp",
                "version": "0.1.0"
            },
            "capabilities": self.capabilities
        });

        Ok(Some(McpMessage::new_response(message.id, result)))
    }

    /// Handle ping request
    async fn handle_ping(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        let result = serde_json::json!({ "timestamp": chrono::Utc::now() });
        Ok(Some(McpMessage::new_response(message.id, result)))
    }

    /// Handle agent registration request
    async fn handle_agent_register(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        // TODO: Implement agent registration logic
        let result = serde_json::json!({ 
            "agent_id": Uuid::new_v4(),
            "status": "registered" 
        });
        Ok(Some(McpMessage::new_response(message.id, result)))
    }

    /// Get connected client count
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}