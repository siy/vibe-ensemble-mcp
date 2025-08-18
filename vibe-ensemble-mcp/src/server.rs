//! MCP server implementation
//!
//! This module provides the core MCP server functionality including
//! protocol handling, capability negotiation, and client session management.

use crate::{protocol::error_codes, protocol::*, Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::agent::{AgentType, ConnectionMetadata};
use vibe_ensemble_core::issue::{IssuePriority, IssueStatus};
use vibe_ensemble_storage::services::{AgentService, IssueService};

/// MCP server state and connection manager
#[derive(Clone)]
pub struct McpServer {
    /// Connected client sessions
    clients: Arc<RwLock<HashMap<String, ClientSession>>>,
    /// Server capabilities
    capabilities: ServerCapabilities,
    /// Agent service for managing agent registration and coordination
    agent_service: Option<Arc<AgentService>>,
    /// Issue service for managing issues and workflows
    issue_service: Option<Arc<IssueService>>,
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
            agent_service: None,
            issue_service: None,
        }
    }

    /// Create a new MCP server with custom capabilities
    pub fn new_with_capabilities(capabilities: ServerCapabilities) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: None,
            issue_service: None,
        }
    }

    /// Create a new MCP server with agent service integration
    pub fn new_with_agent_service(agent_service: Arc<AgentService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(agent_service),
            issue_service: None,
        }
    }

    /// Create a new MCP server with custom capabilities and agent service
    pub fn new_with_capabilities_and_agent_service(
        capabilities: ServerCapabilities,
        agent_service: Arc<AgentService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: Some(agent_service),
            issue_service: None,
        }
    }

    /// Create a new MCP server with issue service integration
    pub fn new_with_issue_service(issue_service: Arc<IssueService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: None,
            issue_service: Some(issue_service),
        }
    }

    /// Create a new MCP server with both agent and issue services
    pub fn new_with_services(
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(agent_service),
            issue_service: Some(issue_service),
        }
    }

    /// Create a new MCP server with custom capabilities and both services
    pub fn new_with_capabilities_and_services(
        capabilities: ServerCapabilities,
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: Some(agent_service),
            issue_service: Some(issue_service),
        }
    }

    /// Handle an incoming JSON-RPC message
    pub async fn handle_message(&self, message: &str) -> Result<Option<String>> {
        debug!("Handling raw message: {}", message);

        // Parse the JSON-RPC message
        let parsed_message: JsonRpcRequest = serde_json::from_str(message).map_err(|e| {
            error!("Failed to parse JSON-RPC message: {}", e);
            Error::Protocol {
                message: format!("Invalid JSON-RPC message: {}", e),
            }
        })?;

        debug!("Parsed JSON-RPC request: {}", parsed_message.method);

        // Handle the request and generate response
        match self.handle_request(parsed_message).await {
            Ok(Some(response)) => {
                let response_json =
                    serde_json::to_string(&response).map_err(Error::Serialization)?;
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
            methods::ISSUE_UPDATE => self.handle_issue_update(request).await,
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
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid initialize parameters: {}", e),
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
            params.client_info.name, params.client_info.version, params.protocol_version
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
                "Vibe Ensemble MCP Server - Coordinating multiple Claude Code instances"
                    .to_string(),
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
    async fn handle_list_resources(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling resources list request");

        let mut resources = vec![
            serde_json::json!({
                "uri": "vibe://agents",
                "name": "Active Agents",
                "description": "List of currently connected Claude Code agents",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "vibe://issues",
                "name": "Open Issues",
                "description": "Currently open issues in the tracking system",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "vibe://knowledge",
                "name": "Knowledge Base",
                "description": "Patterns, practices, and guidelines repository",
                "mimeType": "application/json"
            }),
        ];

        // Add agent-specific resources if agent service is available
        if let Some(agent_service) = &self.agent_service {
            if let Ok(stats) = agent_service.get_statistics().await {
                resources.push(serde_json::json!({
                    "uri": "vibe://agents/online",
                    "name": "Online Agents",
                    "description": format!("Currently online agents ({} total)", stats.online_agents),
                    "mimeType": "application/json"
                }));
                resources.push(serde_json::json!({
                    "uri": "vibe://agents/coordinators",
                    "name": "Coordinator Agents",
                    "description": format!("Coordinator agents ({} total)", stats.coordinator_agents),
                    "mimeType": "application/json"
                }));
                resources.push(serde_json::json!({
                    "uri": "vibe://agents/workers",
                    "name": "Worker Agents",
                    "description": format!("Worker agents ({} total)", stats.worker_agents),
                    "mimeType": "application/json"
                }));
            }
        }

        let result = serde_json::json!({
            "resources": resources
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle prompts list request
    async fn handle_list_prompts(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
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
    async fn handle_agent_register(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent registration request");

        let params: AgentRegisterParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid agent registration parameters: {}", e),
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

        // Check if we have agent service available
        let agent_service = if let Some(service) = &self.agent_service {
            service
        } else {
            warn!("Agent service not available - using fallback registration");
            let result = AgentRegisterResult {
                agent_id: Uuid::new_v4(),
                status: "registered_fallback".to_string(),
                assigned_resources: vec![
                    "vibe://knowledge".to_string(),
                    "vibe://issues".to_string(),
                ],
            };

            return Ok(Some(JsonRpcResponse::success(
                request.id,
                serde_json::to_value(result)?,
            )));
        };

        // Parse agent type
        let agent_type = match params.agent_type.as_str() {
            "Coordinator" => AgentType::Coordinator,
            "Worker" => AgentType::Worker,
            _ => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INVALID_PARAMS,
                        message: format!("Invalid agent type: {}", params.agent_type),
                        data: None,
                    },
                )));
            }
        };

        // Parse connection metadata
        let connection_metadata: ConnectionMetadata =
            serde_json::from_value(params.connection_metadata).map_err(|e| Error::Protocol {
                message: format!("Invalid connection metadata: {}", e),
            })?;

        // Generate session ID for this registration
        let session_id = match &request.id {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => Uuid::new_v4().to_string(),
        };

        // Register the agent using the agent service
        match agent_service
            .register_agent(
                params.name.clone(),
                agent_type,
                params.capabilities,
                connection_metadata,
                session_id.clone(),
            )
            .await
        {
            Ok(agent) => {
                info!(
                    "Successfully registered agent: {} ({})",
                    agent.name, agent.id
                );

                let result = AgentRegisterResult {
                    agent_id: agent.id,
                    status: "registered".to_string(),
                    assigned_resources: vec![
                        "vibe://knowledge".to_string(),
                        "vibe://issues".to_string(),
                        "vibe://agents".to_string(),
                    ],
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
            }
            Err(e) => {
                error!("Failed to register agent {}: {}", params.name, e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_REGISTRATION_FAILED,
                        message: format!("Agent registration failed: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle agent status request
    async fn handle_agent_status(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent status request");

        let result = if let Some(agent_service) = &self.agent_service {
            match agent_service.get_statistics().await {
                Ok(stats) => {
                    serde_json::json!({
                        "total_agents": stats.total_agents,
                        "online_agents": stats.online_agents,
                        "busy_agents": stats.busy_agents,
                        "offline_agents": stats.offline_agents,
                        "coordinator_agents": stats.coordinator_agents,
                        "worker_agents": stats.worker_agents,
                        "active_sessions": stats.active_sessions,
                        "mcp_connections": self.client_count().await
                    })
                }
                Err(e) => {
                    warn!("Failed to get agent statistics: {}", e);
                    serde_json::json!({
                        "connected_agents": self.client_count().await,
                        "active_sessions": self.clients.read().await.len(),
                        "error": "Failed to retrieve agent statistics"
                    })
                }
            }
        } else {
            serde_json::json!({
                "connected_agents": self.client_count().await,
                "active_sessions": self.clients.read().await.len(),
                "note": "Agent service not available"
            })
        };

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle issue creation
    async fn handle_issue_create(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue creation request");
        
        let issue_service = self.issue_service.as_ref().ok_or_else(|| {
            Error::Configuration {
                message: "Issue service not configured".to_string(),
            }
        })?;

        // Parse request parameters
        #[derive(serde::Deserialize)]
        struct CreateIssueParams {
            title: String,
            description: String,
            priority: Option<String>,
            tags: Option<Vec<String>>,
        }

        let params: CreateIssueParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid issue creation parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing issue creation parameters".to_string(),
            });
        };

        // Parse priority
        let priority = match params.priority.as_deref() {
            Some("Low") => IssuePriority::Low,
            Some("Medium") => IssuePriority::Medium,
            Some("High") => IssuePriority::High,
            Some("Critical") => IssuePriority::Critical,
            None => IssuePriority::Medium, // Default
            Some(p) => {
                return Err(Error::Protocol {
                    message: format!("Invalid priority: {}", p),
                });
            }
        };

        let tags = params.tags.unwrap_or_default();

        // Create the issue
        match issue_service.create_issue(params.title, params.description, priority, tags).await {
            Ok(issue) => {
                let result = serde_json::json!({
                    "issue_id": issue.id,
                    "title": issue.title,
                    "description": issue.description,
                    "priority": format!("{:?}", issue.priority),
                    "status": format!("{:?}", issue.status),
                    "tags": issue.tags,
                    "created_at": issue.created_at,
                    "message": "Issue created successfully"
                });
                Ok(Some(JsonRpcResponse::success(request.id, result)))
            }
            Err(e) => {
                error!("Failed to create issue: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to create issue: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle issue update request
    async fn handle_issue_update(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue update request");
        
        let issue_service = self.issue_service.as_ref().ok_or_else(|| {
            Error::Configuration {
                message: "Issue service not configured".to_string(),
            }
        })?;

        // Parse request parameters
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct UpdateIssueParams {
            issue_id: String,
            action: String,
            // Optional fields for different actions
            priority: Option<String>,
            status: Option<String>,
            assigned_agent_id: Option<String>,
            tags: Option<Vec<String>>,
            add_tag: Option<String>,
            remove_tag: Option<String>,
            knowledge_link: Option<String>,
            block_reason: Option<String>,
        }

        let params: UpdateIssueParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid issue update parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing issue update parameters".to_string(),
            });
        };

        let issue_id = Uuid::parse_str(&params.issue_id).map_err(|e| Error::Protocol {
            message: format!("Invalid issue ID: {}", e),
        })?;

        // Perform the requested action
        let result = match params.action.as_str() {
            "assign" => {
                let agent_id_str = params.assigned_agent_id.ok_or_else(|| Error::Protocol {
                    message: "Missing agent ID for assignment".to_string(),
                })?;
                let agent_id = Uuid::parse_str(&agent_id_str).map_err(|e| Error::Protocol {
                    message: format!("Invalid agent ID: {}", e),
                })?;
                match issue_service.assign_issue(issue_id, agent_id).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue assigned successfully",
                        "issue_id": issue.id,
                        "assigned_agent_id": issue.assigned_agent_id,
                        "status": format!("{:?}", issue.status)
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to assign issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "unassign" => {
                match issue_service.unassign_issue(issue_id).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue unassigned successfully",
                        "issue_id": issue.id,
                        "assigned_agent_id": issue.assigned_agent_id,
                        "status": format!("{:?}", issue.status)
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to unassign issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "update_priority" => {
                let priority_str = params.priority.ok_or_else(|| Error::Protocol {
                    message: "Missing priority for priority update".to_string(),
                })?;
                let priority = match priority_str.as_str() {
                    "Low" => IssuePriority::Low,
                    "Medium" => IssuePriority::Medium,
                    "High" => IssuePriority::High,
                    "Critical" => IssuePriority::Critical,
                    _ => return Err(Error::Protocol {
                        message: format!("Invalid priority: {}", priority_str),
                    }),
                };
                match issue_service.update_priority(issue_id, priority).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue priority updated successfully",
                        "issue_id": issue.id,
                        "priority": format!("{:?}", issue.priority)
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to update priority: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "block" => {
                let reason = params.block_reason.ok_or_else(|| Error::Protocol {
                    message: "Missing block reason".to_string(),
                })?;
                match issue_service.block_issue(issue_id, reason).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue blocked successfully",
                        "issue_id": issue.id,
                        "status": match &issue.status {
                            IssueStatus::Blocked { reason } => format!("Blocked: {}", reason),
                            other => format!("{:?}", other),
                        }
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to block issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "unblock" => {
                match issue_service.unblock_issue(issue_id).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue unblocked successfully",
                        "issue_id": issue.id,
                        "status": format!("{:?}", issue.status)
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to unblock issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "resolve" => {
                match issue_service.resolve_issue(issue_id).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue resolved successfully",
                        "issue_id": issue.id,
                        "status": format!("{:?}", issue.status),
                        "resolved_at": issue.resolved_at
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to resolve issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "close" => {
                match issue_service.close_issue(issue_id).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Issue closed successfully",
                        "issue_id": issue.id,
                        "status": format!("{:?}", issue.status),
                        "resolved_at": issue.resolved_at
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to close issue: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "add_tag" => {
                let tag = params.add_tag.ok_or_else(|| Error::Protocol {
                    message: "Missing tag to add".to_string(),
                })?;
                match issue_service.add_tag(issue_id, tag).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Tag added successfully",
                        "issue_id": issue.id,
                        "tags": issue.tags
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to add tag: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "remove_tag" => {
                let tag = params.remove_tag.ok_or_else(|| Error::Protocol {
                    message: "Missing tag to remove".to_string(),
                })?;
                match issue_service.remove_tag(issue_id, &tag).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Tag removed successfully",
                        "issue_id": issue.id,
                        "tags": issue.tags
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to remove tag: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            "add_knowledge_link" => {
                let link = params.knowledge_link.ok_or_else(|| Error::Protocol {
                    message: "Missing knowledge link".to_string(),
                })?;
                match issue_service.add_knowledge_link(issue_id, link).await {
                    Ok(issue) => serde_json::json!({
                        "message": "Knowledge link added successfully",
                        "issue_id": issue.id,
                        "knowledge_links": issue.knowledge_links
                    }),
                    Err(e) => return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to add knowledge link: {}", e),
                            data: None,
                        },
                    ))),
                }
            }
            _ => {
                return Err(Error::Protocol {
                    message: format!("Unknown action: {}", params.action),
                });
            }
        };

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle issue list request
    async fn handle_issue_list(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue list request");
        
        let issue_service = self.issue_service.as_ref().ok_or_else(|| {
            Error::Configuration {
                message: "Issue service not configured".to_string(),
            }
        })?;

        // Parse optional parameters
        #[derive(serde::Deserialize, Default)]
        struct ListIssueParams {
            status: Option<String>,
            priority: Option<String>,
            assigned_agent_id: Option<String>,
            limit: Option<usize>,
        }

        let params: ListIssueParams = if let Some(params) = request.params {
            serde_json::from_value(params).unwrap_or_default()
        } else {
            ListIssueParams::default()
        };

        // Get issues based on filters
        let issues_result = if let Some(status_str) = params.status {
            let status = match status_str.as_str() {
                "Open" => IssueStatus::Open,
                "InProgress" => IssueStatus::InProgress,
                "Resolved" => IssueStatus::Resolved,
                "Closed" => IssueStatus::Closed,
                _ => {
                    if status_str.starts_with("Blocked:") {
                        let reason = status_str.strip_prefix("Blocked:").unwrap_or("").to_string();
                        IssueStatus::Blocked { reason }
                    } else {
                        return Err(Error::Protocol {
                            message: format!("Invalid status filter: {}", status_str),
                        });
                    }
                }
            };
            issue_service.get_issues_by_status(&status).await
        } else if let Some(priority_str) = params.priority {
            let priority = match priority_str.as_str() {
                "Low" => IssuePriority::Low,
                "Medium" => IssuePriority::Medium,
                "High" => IssuePriority::High,
                "Critical" => IssuePriority::Critical,
                _ => {
                    return Err(Error::Protocol {
                        message: format!("Invalid priority filter: {}", priority_str),
                    });
                }
            };
            issue_service.get_issues_by_priority(&priority).await
        } else if let Some(agent_id_str) = params.assigned_agent_id {
            let agent_id = Uuid::parse_str(&agent_id_str).map_err(|e| Error::Protocol {
                message: format!("Invalid agent ID: {}", e),
            })?;
            issue_service.get_agent_issues(agent_id).await
        } else {
            issue_service.list_issues().await
        };

        match issues_result {
            Ok(mut issues) => {
                // Apply limit if specified
                if let Some(limit) = params.limit {
                    issues.truncate(limit);
                }

                let issue_data: Vec<_> = issues.iter().map(|issue| {
                    serde_json::json!({
                        "id": issue.id,
                        "title": issue.title,
                        "description": issue.description,
                        "priority": format!("{:?}", issue.priority),
                        "status": match &issue.status {
                            IssueStatus::Blocked { reason } => format!("Blocked: {}", reason),
                            other => format!("{:?}", other),
                        },
                        "assigned_agent_id": issue.assigned_agent_id,
                        "created_at": issue.created_at,
                        "updated_at": issue.updated_at,
                        "resolved_at": issue.resolved_at,
                        "tags": issue.tags,
                        "knowledge_links": issue.knowledge_links,
                        "is_assigned": issue.is_assigned(),
                        "is_terminal": issue.is_terminal(),
                        "age_seconds": issue.age_seconds(),
                    })
                }).collect();

                let result = serde_json::json!({
                    "issues": issue_data,
                    "total": issues.len()
                });
                Ok(Some(JsonRpcResponse::success(request.id, result)))
            }
            Err(e) => {
                error!("Failed to list issues: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to list issues: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle knowledge query
    async fn handle_knowledge_query(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
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
        let removed = self.clients.write().await.remove(client_id).is_some();

        // If we have agent service, handle agent deregistration
        if removed {
            if let Some(agent_service) = &self.agent_service {
                // Try to parse client_id as UUID for agent deregistration
                if let Ok(agent_id) = Uuid::parse_str(client_id) {
                    if let Err(e) = agent_service.deregister_agent(agent_id).await {
                        warn!(
                            "Failed to deregister agent {} on disconnect: {}",
                            agent_id, e
                        );
                    } else {
                        info!("Successfully deregistered agent {} on disconnect", agent_id);
                    }
                }
            }
        }

        removed
    }

    /// Get all connected client IDs
    pub async fn connected_clients(&self) -> Vec<String> {
        self.clients.read().await.keys().cloned().collect()
    }

    /// Get agent service (if available)
    pub fn agent_service(&self) -> Option<Arc<AgentService>> {
        self.agent_service.clone()
    }

    /// Update agent heartbeat (health check)
    pub async fn update_agent_heartbeat(&self, agent_id: Uuid) -> Result<()> {
        if let Some(agent_service) = &self.agent_service {
            agent_service.update_heartbeat(agent_id).await?;
        }
        Ok(())
    }

    /// Cleanup stale agent sessions
    pub async fn cleanup_stale_agents(&self, max_idle_seconds: i64) -> Result<Vec<Uuid>> {
        if let Some(agent_service) = &self.agent_service {
            agent_service
                .cleanup_stale_sessions(max_idle_seconds)
                .await
                .map_err(crate::Error::Storage)
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
