//! MCP server implementation
//!
//! This module provides the core MCP server functionality including
//! protocol handling, capability negotiation, and client session management.

use crate::{
    protocol::*, 
    protocol::error_codes,
    Error, 
    Result
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// MCP server state and connection manager
#[derive(Clone)]
pub struct McpServer {
    /// Connected client sessions
    clients: Arc<RwLock<HashMap<String, ClientSession>>>,
    /// Server capabilities
    capabilities: ServerCapabilities,
}

/// Client session information
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub id: String,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub protocol_version: String,
}

impl McpServer {
    /// Create a new MCP server with default capabilities
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
        }
    }

    /// Create a new MCP server with custom capabilities
    pub fn new_with_capabilities(capabilities: ServerCapabilities) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
        }
    }

    /// Handle an incoming JSON-RPC message
    pub async fn handle_message(&self, message: &str) -> Result<Option<String>> {
        debug!("Handling raw message: {}", message);

        // Parse the JSON-RPC message
        let parsed_message: JsonRpcRequest = serde_json::from_str(message)
            .map_err(|e| {
                error!("Failed to parse JSON-RPC message: {}", e);
                Error::Protocol {
                    message: format!("Invalid JSON-RPC message: {}", e),
                }
            })?;

        debug!("Parsed JSON-RPC request: {}", parsed_message.method);

        // Handle the request and generate response
        match self.handle_request(parsed_message).await {
            Ok(Some(response)) => {
                let response_json = serde_json::to_string(&response)
                    .map_err(Error::Serialization)?;
                Ok(Some(response_json))
            }
            Ok(None) => Ok(None), // No response needed (notification)
            Err(e) => {
                error!("Error handling request: {}", e);
                Err(e)
            }
        }
    }

    /// Handle a JSON-RPC request and return a response
    async fn handle_request(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        match request.method.as_str() {
            methods::INITIALIZE => self.handle_initialize(request).await,
            methods::PING => self.handle_ping(request).await,
            methods::LIST_TOOLS => self.handle_list_tools(request).await,
            methods::LIST_RESOURCES => self.handle_list_resources(request).await,
            methods::LIST_PROMPTS => self.handle_list_prompts(request).await,
            
            // Vibe Ensemble extensions
            methods::AGENT_REGISTER => self.handle_agent_register(request).await,
            methods::AGENT_STATUS => self.handle_agent_status(request).await,
            methods::ISSUE_CREATE => self.handle_issue_create(request).await,
            methods::ISSUE_LIST => self.handle_issue_list(request).await,
            methods::KNOWLEDGE_QUERY => self.handle_knowledge_query(request).await,
            
            _ => {
                warn!("Unknown method: {}", request.method);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::METHOD_NOT_FOUND,
                        message: "Method not found".to_string(),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle MCP initialization
    async fn handle_initialize(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        let params: InitializeParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| {
                Error::InvalidParams {
                    message: format!("Invalid initialize parameters: {}", e),
                }
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing initialize parameters".to_string(),
                    data: None,
                },
            )));
        };

        info!(
            "Client initializing: {} v{} (protocol: {})",
            params.client_info.name, 
            params.client_info.version,
            params.protocol_version
        );

        // Validate protocol version - warning only for now as we support backwards compatibility
        if params.protocol_version != MCP_VERSION {
            warn!(
                "Protocol version mismatch: client={}, server={} - proceeding with connection",
                params.protocol_version, MCP_VERSION
            );
        }

        // Create client session
        let session_id = match &request.id {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => Uuid::new_v4().to_string(),
        };

        let session = ClientSession {
            id: session_id.clone(),
            client_info: params.client_info,
            capabilities: params.capabilities,
            connected_at: chrono::Utc::now(),
            protocol_version: params.protocol_version,
        };

        // Store the session
        self.clients.write().await.insert(session_id, session);

        // Create initialization response
        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            server_info: ServerInfo {
                name: "vibe-ensemble-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: self.capabilities.clone(),
            instructions: Some(
                "Vibe Ensemble MCP Server - Coordinating multiple Claude Code instances".to_string()
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle ping request for connection health check
    async fn handle_ping(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling ping request");
        
        let result = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "server": "vibe-ensemble-mcp",
            "version": "0.1.0"
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle tools list request
    async fn handle_list_tools(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling tools list request");
        
        let result = serde_json::json!({
            "tools": [
                {
                    "name": "agent_register",
                    "description": "Register a new Claude Code agent with the system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "agentType": {"type": "string"},
                            "capabilities": {"type": "array", "items": {"type": "string"}},
                            "connectionMetadata": {"type": "object"}
                        },
                        "required": ["name", "agentType", "capabilities"]
                    }
                },
                {
                    "name": "issue_create",
                    "description": "Create a new issue in the tracking system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string"},
                            "description": {"type": "string"},
                            "priority": {"type": "string", "enum": ["Low", "Medium", "High", "Critical"]}
                        },
                        "required": ["title", "description"]
                    }
                }
            ]
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle resources list request
    async fn handle_list_resources(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling resources list request");
        
        let result = serde_json::json!({
            "resources": [
                {
                    "uri": "vibe://agents",
                    "name": "Active Agents",
                    "description": "List of currently connected Claude Code agents",
                    "mimeType": "application/json"
                },
                {
                    "uri": "vibe://issues",
                    "name": "Open Issues",
                    "description": "Currently open issues in the tracking system",
                    "mimeType": "application/json"
                },
                {
                    "uri": "vibe://knowledge",
                    "name": "Knowledge Base",
                    "description": "Patterns, practices, and guidelines repository",
                    "mimeType": "application/json"
                }
            ]
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle prompts list request
    async fn handle_list_prompts(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling prompts list request");
        
        let result = serde_json::json!({
            "prompts": [
                {
                    "name": "coordinator_prompt",
                    "description": "System prompt for Claude Code Team Coordinator",
                    "arguments": [
                        {
                            "name": "task_type",
                            "description": "Type of coordination task",
                            "required": false
                        }
                    ]
                },
                {
                    "name": "worker_prompt",
                    "description": "System prompt for Claude Code Worker agents",
                    "arguments": [
                        {
                            "name": "capability",
                            "description": "Primary capability focus",
                            "required": true
                        }
                    ]
                }
            ]
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle agent registration (Vibe Ensemble extension)
    async fn handle_agent_register(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent registration request");
        
        let params: AgentRegisterParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| {
                Error::Protocol {
                    message: format!("Invalid agent registration parameters: {}", e),
                }
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing agent registration parameters".to_string(),
                    data: None,
                },
            )));
        };

        info!(
            "Registering agent: {} (type: {}, capabilities: {:?})",
            params.name, params.agent_type, params.capabilities
        );

        // TODO: Integrate with agent management system from vibe-ensemble-core
        let result = AgentRegisterResult {
            agent_id: Uuid::new_v4(),
            status: "registered".to_string(),
            assigned_resources: vec![
                "vibe://knowledge".to_string(),
                "vibe://issues".to_string(),
            ],
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle agent status request
    async fn handle_agent_status(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent status request");
        
        // TODO: Implement actual agent status retrieval
        let result = serde_json::json!({
            "connected_agents": self.client_count().await,
            "active_sessions": self.clients.read().await.len()
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle issue creation
    async fn handle_issue_create(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue creation request");
        
        // TODO: Integrate with issue tracking system from vibe-ensemble-core
        let result = serde_json::json!({
            "issue_id": Uuid::new_v4(),
            "status": "created",
            "message": "Issue created successfully"
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle issue list request
    async fn handle_issue_list(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue list request");
        
        // TODO: Integrate with issue tracking system
        let result = serde_json::json!({
            "issues": [],
            "total": 0
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle knowledge query
    async fn handle_knowledge_query(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling knowledge query request");
        
        // TODO: Integrate with knowledge management system
        let result = serde_json::json!({
            "results": [],
            "total": 0
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Get server capabilities
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Check if a client is connected
    pub async fn is_client_connected(&self, client_id: &str) -> bool {
        self.clients.read().await.contains_key(client_id)
    }

    /// Disconnect a client
    pub async fn disconnect_client(&self, client_id: &str) -> bool {
        self.clients.write().await.remove(client_id).is_some()
    }

    /// Get all connected client IDs
    pub async fn connected_clients(&self) -> Vec<String> {
        self.clients.read().await.keys().cloned().collect()
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}