//! MCP server implementation
//!
//! This module provides the core MCP server functionality including
//! protocol handling, capability negotiation, and client session management.

use crate::{
    protocol::{
        error_codes, AgentDeregisterParams, AgentDeregisterResult, AgentListParams,
        AgentStatusParams, ConflictResolveParams, ConflictResolveResult,
        CoordinatorRequestWorkerParams, CoordinatorRequestWorkerResult, DependencyDeclareParams,
        DependencyDeclareResult, IssueAssignParams, IssueAssignResult, IssueCloseParams,
        IssueCloseResult, IssueCreateParams, IssueCreateResult, IssueInfo, IssueListParams,
        IssueListResult, IssueUpdateParams, IssueUpdateResult, ProjectLockParams,
        ProjectLockResult, WorkCoordinateParams, WorkCoordinateResult, WorkerCoordinateParams,
        WorkerCoordinateResult, WorkerMessageParams, WorkerMessageResult, WorkerRequestParams,
        WorkerRequestResult, *,
    },
    Error, Result,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::agent::{AgentStatus, AgentType, ConnectionMetadata};
use vibe_ensemble_core::issue::{IssuePriority, IssueStatus};
use vibe_ensemble_core::message::{MessagePriority, MessageType};
use vibe_ensemble_storage::services::{
    AgentService, CoordinationService, IssueService, MessageService,
};

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
    /// Message service for real-time messaging
    message_service: Option<Arc<MessageService>>,
    /// Coordination service for cross-project dependencies and worker coordination
    coordination_service: Option<Arc<CoordinationService>>,
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
            message_service: None,
            coordination_service: None,
        }
    }

    /// Create a new MCP server with custom capabilities
    pub fn new_with_capabilities(capabilities: ServerCapabilities) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: None,
            issue_service: None,
            message_service: None,
            coordination_service: None,
        }
    }

    /// Create a new MCP server with agent service integration
    pub fn new_with_agent_service(agent_service: Arc<AgentService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(agent_service),
            issue_service: None,
            message_service: None,
            coordination_service: None,
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
            message_service: None,
            coordination_service: None,
        }
    }

    /// Create a new MCP server with issue service integration
    pub fn new_with_issue_service(issue_service: Arc<IssueService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: None,
            issue_service: Some(issue_service),
            message_service: None,
            coordination_service: None,
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
            message_service: None,
            coordination_service: None,
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
            message_service: None,
            coordination_service: None,
        }
    }

    /// Create a new MCP server with message service integration
    pub fn new_with_message_service(message_service: Arc<MessageService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: None,
            issue_service: None,
            message_service: Some(message_service),
            coordination_service: None,
        }
    }

    /// Create a new MCP server with all services
    pub fn new_with_all_services(
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(agent_service),
            issue_service: Some(issue_service),
            message_service: Some(message_service),
            coordination_service: None,
        }
    }

    /// Create a new MCP server with custom capabilities and all services
    pub fn new_with_capabilities_and_all_services(
        capabilities: ServerCapabilities,
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: Some(agent_service),
            issue_service: Some(issue_service),
            message_service: Some(message_service),
            coordination_service: None,
        }
    }

    /// Create a new MCP server with coordination service
    pub fn new_with_coordination_service(coordination_service: Arc<CoordinationService>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: None,
            issue_service: None,
            message_service: None,
            coordination_service: Some(coordination_service),
        }
    }

    /// Create a new MCP server with all services including coordination
    pub fn new_with_full_services(
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
        message_service: Arc<MessageService>,
        coordination_service: Arc<CoordinationService>,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(agent_service),
            issue_service: Some(issue_service),
            message_service: Some(message_service),
            coordination_service: Some(coordination_service),
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
        let request_id = parsed_message.id.clone();
        match self.handle_request(parsed_message).await {
            Ok(Some(response)) => {
                let response_json =
                    serde_json::to_string(&response).map_err(Error::Serialization)?;
                Ok(Some(response_json))
            }
            Ok(None) => Ok(None), // No response needed (notification)
            Err(e) => {
                error!("Error handling request: {}", e);

                // Convert error to JSON-RPC error response
                let error_code = match &e {
                    Error::Protocol { .. } => error_codes::INVALID_PARAMS,
                    Error::InvalidParams { .. } => error_codes::INVALID_PARAMS,
                    Error::Configuration { .. } => error_codes::INTERNAL_ERROR,
                    _ => error_codes::INTERNAL_ERROR,
                };

                let error_response = JsonRpcResponse::error(
                    request_id,
                    JsonRpcError {
                        code: error_code,
                        message: e.to_string(),
                        data: None,
                    },
                );

                let response_json =
                    serde_json::to_string(&error_response).map_err(Error::Serialization)?;
                Ok(Some(response_json))
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
            methods::AGENT_LIST => self.handle_agent_list(request).await,
            methods::AGENT_DEREGISTER => self.handle_agent_deregister(request).await,
            methods::ISSUE_CREATE => self.handle_issue_create_new(request).await,
            methods::ISSUE_LIST => self.handle_issue_list_new(request).await,
            methods::ISSUE_ASSIGN => self.handle_issue_assign(request).await,
            methods::ISSUE_UPDATE => self.handle_issue_update_new(request).await,
            methods::ISSUE_CLOSE => self.handle_issue_close(request).await,
            methods::MESSAGE_SEND => self.handle_message_send(request).await,
            methods::MESSAGE_BROADCAST => self.handle_message_broadcast(request).await,
            methods::KNOWLEDGE_QUERY => self.handle_knowledge_query(request).await,

            // Worker communication methods
            methods::WORKER_MESSAGE => self.handle_worker_message(request).await,
            methods::WORKER_REQUEST => self.handle_worker_request(request).await,
            methods::WORKER_COORDINATE => self.handle_worker_coordinate(request).await,
            methods::PROJECT_LOCK => self.handle_project_lock(request).await,

            // Cross-project dependency coordination methods
            methods::DEPENDENCY_DECLARE => self.handle_dependency_declare(request).await,
            methods::COORDINATOR_REQUEST_WORKER => {
                self.handle_coordinator_request_worker(request).await
            }
            methods::WORK_COORDINATE => self.handle_work_coordinate(request).await,
            methods::CONFLICT_RESOLVE => self.handle_conflict_resolve(request).await,

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
                    "name": "vibe_agent_register",
                    "description": "Register a new Claude Code agent with the system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "Agent name"},
                            "agentType": {"type": "string", "enum": ["Coordinator", "Worker"], "description": "Agent type"},
                            "capabilities": {"type": "array", "items": {"type": "string"}, "description": "Agent capabilities"},
                            "connectionMetadata": {"type": "object", "description": "Connection metadata"}
                        },
                        "required": ["name", "agentType", "capabilities"]
                    }
                },
                {
                    "name": "vibe_agent_status",
                    "description": "Report agent status or query system statistics",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agentId": {"type": "string", "description": "Agent ID (for status updates)"},
                            "status": {"type": "string", "enum": ["Connecting", "Online", "Idle", "Busy", "Maintenance", "Disconnecting", "Offline"], "description": "Agent status"},
                            "currentTask": {"type": "string", "description": "Current task description"},
                            "progress": {"type": "number", "minimum": 0, "maximum": 1, "description": "Task progress (0-1)"},
                            "healthMetrics": {"type": "object", "description": "Health metrics data"}
                        },
                        "required": []
                    }
                },
                {
                    "name": "vibe_agent_list",
                    "description": "List active agents with optional filtering",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": {"type": "string", "description": "Filter by project"},
                            "capability": {"type": "string", "description": "Filter by capability"},
                            "status": {"type": "string", "enum": ["Connecting", "Online", "Idle", "Busy", "Maintenance", "Disconnecting", "Offline"], "description": "Filter by status"},
                            "agentType": {"type": "string", "enum": ["Coordinator", "Worker"], "description": "Filter by agent type"},
                            "limit": {"type": "integer", "minimum": 1, "description": "Maximum number of agents to return"}
                        },
                        "required": []
                    }
                },
                {
                    "name": "vibe_agent_deregister",
                    "description": "Deregister an agent from the system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agentId": {"type": "string", "description": "Agent ID to deregister"},
                            "shutdownReason": {"type": "string", "description": "Reason for shutdown"}
                        },
                        "required": ["agentId"]
                    }
                },
                {
                    "name": "vibe_issue_create",
                    "description": "Create a new issue in the tracking system",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string", "description": "Issue title"},
                            "description": {"type": "string", "description": "Issue description"},
                            "priority": {"type": "string", "enum": ["Low", "Medium", "High", "Critical"], "description": "Issue priority"},
                            "issueType": {"type": "string", "description": "Type of issue (e.g., bug, feature, task)"},
                            "projectId": {"type": "string", "description": "Project identifier"},
                            "createdByAgentId": {"type": "string", "description": "ID of the agent creating the issue"},
                            "labels": {"type": "array", "items": {"type": "string"}, "description": "Issue labels/tags"},
                            "assignee": {"type": "string", "description": "Agent ID to assign the issue to"}
                        },
                        "required": ["title", "description", "createdByAgentId"]
                    }
                },
                {
                    "name": "vibe_issue_list",
                    "description": "Query issues by project/status/assignee with comprehensive filtering",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "projectId": {"type": "string", "description": "Filter by project ID"},
                            "status": {"type": "string", "enum": ["Open", "InProgress", "Resolved", "Closed"], "description": "Filter by status"},
                            "assignee": {"type": "string", "description": "Filter by assignee agent ID"},
                            "issueType": {"type": "string", "description": "Filter by issue type"},
                            "priority": {"type": "string", "enum": ["Low", "Medium", "High", "Critical"], "description": "Filter by priority"},
                            "labels": {"type": "array", "items": {"type": "string"}, "description": "Filter by labels"},
                            "limit": {"type": "integer", "minimum": 1, "description": "Maximum number of issues to return"}
                        },
                        "required": []
                    }
                },
                {
                    "name": "vibe_issue_assign",
                    "description": "Assign issues to workers or coordinator",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "issueId": {"type": "string", "description": "ID of the issue to assign"},
                            "assigneeAgentId": {"type": "string", "description": "Agent ID to assign the issue to"},
                            "assignedByAgentId": {"type": "string", "description": "Agent ID performing the assignment"},
                            "reason": {"type": "string", "description": "Reason for assignment"}
                        },
                        "required": ["issueId", "assigneeAgentId", "assignedByAgentId"]
                    }
                },
                {
                    "name": "vibe_issue_update",
                    "description": "Update issue status and add comments",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "issueId": {"type": "string", "description": "ID of the issue to update"},
                            "status": {"type": "string", "enum": ["Open", "InProgress", "Resolved", "Closed"], "description": "New status"},
                            "comment": {"type": "string", "description": "Comment to add to the issue"},
                            "updatedByAgentId": {"type": "string", "description": "Agent ID performing the update"},
                            "priority": {"type": "string", "enum": ["Low", "Medium", "High", "Critical"], "description": "Updated priority"}
                        },
                        "required": ["issueId", "updatedByAgentId"]
                    }
                },
                {
                    "name": "vibe_issue_close",
                    "description": "Mark issues as resolved/closed",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "issueId": {"type": "string", "description": "ID of the issue to close"},
                            "closedByAgentId": {"type": "string", "description": "Agent ID closing the issue"},
                            "resolution": {"type": "string", "description": "Resolution description"},
                            "closeReason": {"type": "string", "description": "Reason for closing"}
                        },
                        "required": ["issueId", "closedByAgentId", "resolution"]
                    }
                },
                {
                    "name": "vibe_worker_message",
                    "description": "Send direct messages between workers for real-time coordination",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "recipientAgentId": {"type": "string", "description": "Agent ID of the message recipient"},
                            "messageContent": {"type": "string", "description": "Content of the message"},
                            "messageType": {"type": "string", "enum": ["Info", "Request", "Coordination", "Alert"], "description": "Type of message"},
                            "senderAgentId": {"type": "string", "description": "Agent ID of the message sender"},
                            "priority": {"type": "string", "enum": ["Low", "Normal", "High", "Urgent"], "description": "Message priority"},
                            "metadata": {"type": "object", "description": "Additional message metadata"}
                        },
                        "required": ["recipientAgentId", "messageContent", "messageType", "senderAgentId"]
                    }
                },
                {
                    "name": "vibe_worker_request",
                    "description": "Request specific actions from targeted workers",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "targetAgentId": {"type": "string", "description": "Agent ID of the request target"},
                            "requestType": {"type": "string", "description": "Type of request being made"},
                            "requestDetails": {"type": "object", "description": "Detailed request information"},
                            "requestedByAgentId": {"type": "string", "description": "Agent ID making the request"},
                            "deadline": {"type": "string", "format": "date-time", "description": "Request deadline"},
                            "priority": {"type": "string", "enum": ["Low", "Normal", "High", "Urgent"], "description": "Request priority"}
                        },
                        "required": ["targetAgentId", "requestType", "requestDetails", "requestedByAgentId"]
                    }
                },
                {
                    "name": "vibe_worker_coordinate",
                    "description": "Coordinate overlapping work areas between multiple workers",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "coordinationType": {"type": "string", "description": "Type of coordination needed"},
                            "involvedAgents": {"type": "array", "items": {"type": "string"}, "description": "Agent IDs involved in coordination"},
                            "scope": {"type": "object", "description": "Coordination scope (files/modules/projects)"},
                            "coordinatorAgentId": {"type": "string", "description": "Agent ID initiating coordination"},
                            "details": {"type": "object", "description": "Coordination details and requirements"}
                        },
                        "required": ["coordinationType", "involvedAgents", "scope", "coordinatorAgentId", "details"]
                    }
                },
                {
                    "name": "vibe_project_lock",
                    "description": "Create project-level coordination locks to prevent conflicts",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "projectId": {"type": "string", "description": "Project identifier (optional)"},
                            "resourcePath": {"type": "string", "description": "Path to resource being locked"},
                            "lockType": {"type": "string", "enum": ["Exclusive", "Shared", "Coordination"], "description": "Type of lock"},
                            "lockHolderAgentId": {"type": "string", "description": "Agent ID requesting the lock"},
                            "duration": {"type": "integer", "description": "Lock duration in seconds"},
                            "reason": {"type": "string", "description": "Reason for the lock"}
                        },
                        "required": ["resourcePath", "lockType", "lockHolderAgentId", "reason"]
                    }
                },
                {
                    "name": "vibe_dependency_declare",
                    "description": "Declare a cross-project dependency and create coordination plan",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "declaringAgentId": {"type": "string", "description": "ID of agent declaring dependency"},
                            "sourceProject": {"type": "string", "description": "Source project name"},
                            "targetProject": {"type": "string", "description": "Target project name"},
                            "dependencyType": {"type": "string", "enum": ["API_CHANGE", "SHARED_RESOURCE", "BUILD_DEPENDENCY", "CONFIGURATION", "DATA_SCHEMA"], "description": "Type of dependency"},
                            "description": {"type": "string", "description": "Description of dependency"},
                            "impact": {"type": "string", "enum": ["BLOCKER", "MAJOR", "MINOR", "INFO"], "description": "Impact level"},
                            "urgency": {"type": "string", "enum": ["CRITICAL", "HIGH", "MEDIUM", "LOW"], "description": "Urgency level"},
                            "affectedFiles": {"type": "array", "items": {"type": "string"}, "description": "List of affected files"},
                            "metadata": {"type": "object", "description": "Additional metadata"}
                        },
                        "required": ["declaringAgentId", "sourceProject", "targetProject", "dependencyType", "description", "impact", "urgency"]
                    }
                },
                {
                    "name": "vibe_coordinator_request_worker",
                    "description": "Request coordinator to spawn a new worker for a project",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "requestingAgentId": {"type": "string", "description": "ID of requesting agent"},
                            "targetProject": {"type": "string", "description": "Target project name"},
                            "requiredCapabilities": {"type": "array", "items": {"type": "string"}, "description": "Required worker capabilities"},
                            "priority": {"type": "string", "enum": ["CRITICAL", "HIGH", "MEDIUM", "LOW"], "description": "Spawn priority"},
                            "taskDescription": {"type": "string", "description": "Task description for new worker"},
                            "estimatedDuration": {"type": "string", "description": "Estimated duration (e.g., '2h', '30m')"},
                            "contextData": {"type": "object", "description": "Context data for worker"}
                        },
                        "required": ["requestingAgentId", "targetProject", "requiredCapabilities", "priority", "taskDescription"]
                    }
                },
                {
                    "name": "vibe_work_coordinate",
                    "description": "Negotiate work ordering and coordination between agents",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "initiatingAgentId": {"type": "string", "description": "ID of initiating agent"},
                            "targetAgentId": {"type": "string", "description": "ID of target agent"},
                            "coordinationType": {"type": "string", "enum": ["SEQUENTIAL", "PARALLEL", "BLOCKING", "COLLABORATIVE", "CONFLICT_RESOLUTION"], "description": "Type of coordination"},
                            "workItems": {"type": "array", "items": {"type": "object"}, "description": "List of work items to coordinate"},
                            "dependencies": {"type": "array", "items": {"type": "object"}, "description": "Work dependencies"},
                            "proposedTimeline": {"type": "object", "description": "Proposed coordination timeline"},
                            "resourceRequirements": {"type": "object", "description": "Resource requirements"}
                        },
                        "required": ["initiatingAgentId", "targetAgentId", "coordinationType", "workItems"]
                    }
                },
                {
                    "name": "vibe_conflict_resolve",
                    "description": "Resolve conflicts between agents working on overlapping resources",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "affectedAgents": {"type": "array", "items": {"type": "string"}, "description": "IDs of affected agents"},
                            "conflictedResources": {"type": "array", "items": {"type": "string"}, "description": "List of conflicted resources"},
                            "conflictType": {"type": "string", "enum": ["FILE_MODIFICATION", "RESOURCE_LOCK", "ARCHITECTURE", "BUSINESS_LOGIC", "TESTING", "DEPLOYMENT"], "description": "Type of conflict"},
                            "resolutionStrategy": {"type": "string", "enum": ["LAST_WRITER_WINS", "FIRST_WRITER_WINS", "AUTO_MERGE", "MANUAL_MERGE", "RESOURCE_SPLIT", "SEQUENTIAL", "ESCALATE"], "description": "Preferred resolution strategy"},
                            "resolverAgentId": {"type": "string", "description": "ID of agent handling resolution"},
                            "conflictEvidence": {"type": "array", "items": {"type": "object"}, "description": "Evidence of the conflict"},
                            "priority": {"type": "string", "enum": ["CRITICAL", "HIGH", "MEDIUM", "LOW"], "description": "Resolution priority"}
                        },
                        "required": ["affectedAgents", "conflictedResources", "conflictType", "resolverAgentId"]
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

    /// Handle agent status request - supports both reporting status and querying status
    async fn handle_agent_status(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent status request");

        let agent_service = if let Some(service) = &self.agent_service {
            service
        } else {
            // Fallback: return system-level statistics
            let result = serde_json::json!({
                "connected_agents": self.client_count().await,
                "active_sessions": self.clients.read().await.len(),
                "note": "Agent service not available"
            });
            return Ok(Some(JsonRpcResponse::success(request.id, result)));
        };

        // Check if this is a status update (has parameters) or status query (no parameters)
        if let Some(params) = request.params {
            // Status update from an agent
            let status_params: AgentStatusParams =
                serde_json::from_value(params).map_err(|e| Error::Protocol {
                    message: format!("Invalid agent status parameters: {}", e),
                })?;

            let agent_id =
                Uuid::parse_str(&status_params.agent_id).map_err(|e| Error::Protocol {
                    message: format!("Invalid agent ID: {}", e),
                })?;

            // Update heartbeat
            if let Err(e) = agent_service.update_heartbeat(agent_id).await {
                warn!("Failed to update heartbeat for agent {}: {}", agent_id, e);
            }

            // Parse and update status if provided
            if let Ok(agent_status) = self.parse_agent_status(&status_params.status) {
                if let Err(e) = agent_service
                    .update_agent_status(agent_id, agent_status)
                    .await
                {
                    warn!("Failed to update status for agent {}: {}", agent_id, e);
                }
            }

            // Return acknowledgment
            let result = serde_json::json!({
                "agent_id": agent_id,
                "status": "acknowledged",
                "timestamp": chrono::Utc::now(),
                "message": "Status update received"
            });
            Ok(Some(JsonRpcResponse::success(request.id, result)))
        } else {
            // Status query - return system-wide statistics
            match agent_service.get_statistics().await {
                Ok(stats) => {
                    let result = serde_json::json!({
                        "total_agents": stats.total_agents,
                        "online_agents": stats.online_agents,
                        "busy_agents": stats.busy_agents,
                        "offline_agents": stats.offline_agents,
                        "coordinator_agents": stats.coordinator_agents,
                        "worker_agents": stats.worker_agents,
                        "active_sessions": stats.active_sessions,
                        "mcp_connections": self.client_count().await
                    });
                    Ok(Some(JsonRpcResponse::success(request.id, result)))
                }
                Err(e) => {
                    warn!("Failed to get agent statistics: {}", e);
                    let result = serde_json::json!({
                        "connected_agents": self.client_count().await,
                        "active_sessions": self.clients.read().await.len(),
                        "error": "Failed to retrieve agent statistics"
                    });
                    Ok(Some(JsonRpcResponse::success(request.id, result)))
                }
            }
        }
    }

    /// Parse agent status string into AgentStatus enum
    fn parse_agent_status(&self, status_str: &str) -> Result<AgentStatus> {
        match status_str {
            "Connecting" => Ok(AgentStatus::Connecting),
            "Online" => Ok(AgentStatus::Online),
            "Idle" => Ok(AgentStatus::Idle),
            "Busy" => Ok(AgentStatus::Busy),
            "Maintenance" => Ok(AgentStatus::Maintenance),
            "Disconnecting" => Ok(AgentStatus::Disconnecting),
            "Offline" => Ok(AgentStatus::Offline),
            s if s.starts_with("Error:") => {
                let message = s.strip_prefix("Error:").unwrap_or("").trim().to_string();
                Ok(AgentStatus::Error { message })
            }
            s if s.starts_with("Unhealthy:") => {
                let reason = s
                    .strip_prefix("Unhealthy:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                Ok(AgentStatus::Unhealthy { reason })
            }
            _ => Err(Error::Protocol {
                message: format!("Invalid agent status: {}", status_str),
            }),
        }
    }

    /// Handle agent list request
    async fn handle_agent_list(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent list request");

        let agent_service = if let Some(service) = &self.agent_service {
            service
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INTERNAL_ERROR,
                    message: "Agent service not available".to_string(),
                    data: None,
                },
            )));
        };

        // Parse optional filter parameters
        let params: AgentListParams = if let Some(params) = request.params {
            serde_json::from_value(params).unwrap_or_default()
        } else {
            AgentListParams::default()
        };

        // Get agents based on filters
        let agents_result = if let Some(capability) = &params.capability {
            agent_service.find_agents_by_capability(capability).await
        } else if let Some(agent_type_str) = &params.agent_type {
            let agent_type = match agent_type_str.as_str() {
                "Coordinator" => AgentType::Coordinator,
                "Worker" => AgentType::Worker,
                _ => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid agent type: {}", agent_type_str),
                            data: None,
                        },
                    )));
                }
            };
            agent_service.list_agents_by_type(&agent_type).await
        } else if let Some(status_str) = &params.status {
            let status = match self.parse_agent_status(status_str) {
                Ok(status) => status,
                Err(_) => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid status filter: {}", status_str),
                            data: None,
                        },
                    )));
                }
            };
            match status {
                AgentStatus::Online => agent_service.list_online_agents().await,
                _ => {
                    // For non-online statuses, use the generic method from repository
                    let all_agents = agent_service.list_agents().await?;
                    Ok(all_agents
                        .into_iter()
                        .filter(|agent| {
                            std::mem::discriminant(&agent.status) == std::mem::discriminant(&status)
                        })
                        .collect())
                }
            }
        } else {
            agent_service.list_agents().await
        };

        match agents_result {
            Ok(mut agents) => {
                // Apply limit if specified
                if let Some(limit) = params.limit {
                    agents.truncate(limit);
                }

                let agent_data: Vec<_> = agents
                    .iter()
                    .map(|agent| {
                        serde_json::json!({
                            "id": agent.id,
                            "name": agent.name,
                            "agent_type": format!("{:?}", agent.agent_type),
                            "status": self.format_agent_status(&agent.status),
                            "capabilities": agent.capabilities,
                            "connected_at": agent.created_at,
                            "last_seen": agent.last_seen,
                            "is_healthy": agent.is_healthy(60), // 60-second health check
                            "is_available": agent.is_available(),
                            "performance_score": agent.performance_metrics.success_rate(),
                            "current_tasks": agent.resource_allocation.current_task_count,
                            "max_tasks": agent.resource_allocation.max_concurrent_tasks,
                            "load_factor": agent.resource_allocation.load_factor
                        })
                    })
                    .collect();

                let result = serde_json::json!({
                    "agents": agent_data,
                    "total": agents.len(),
                    "filters_applied": {
                        "capability": params.capability,
                        "agent_type": params.agent_type,
                        "status": params.status,
                        "project": params.project,
                        "limit": params.limit
                    }
                });

                Ok(Some(JsonRpcResponse::success(request.id, result)))
            }
            Err(e) => {
                error!("Failed to list agents: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to list agents: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle agent deregistration request
    async fn handle_agent_deregister(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling agent deregistration request");

        let agent_service = if let Some(service) = &self.agent_service {
            service
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INTERNAL_ERROR,
                    message: "Agent service not available".to_string(),
                    data: None,
                },
            )));
        };

        // Parse deregistration parameters
        let params: AgentDeregisterParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid agent deregistration parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing agent deregistration parameters".to_string(),
                    data: None,
                },
            )));
        };

        let agent_id = Uuid::parse_str(&params.agent_id).map_err(|e| Error::Protocol {
            message: format!("Invalid agent ID: {}", e),
        })?;

        info!(
            "Deregistering agent: {} (reason: {:?})",
            agent_id, params.shutdown_reason
        );

        // Verify agent exists before attempting deregistration
        match agent_service.get_agent(agent_id).await {
            Ok(Some(_agent)) => {
                // Proceed with deregistration
                match agent_service.deregister_agent(agent_id).await {
                    Ok(()) => {
                        // Remove from active sessions
                        self.disconnect_client(&agent_id.to_string()).await;

                        let result = AgentDeregisterResult {
                            agent_id,
                            status: "deregistered".to_string(),
                            cleanup_status: "completed".to_string(),
                        };

                        info!("Successfully deregistered agent: {}", agent_id);
                        Ok(Some(JsonRpcResponse::success(
                            request.id,
                            serde_json::to_value(result)?,
                        )))
                    }
                    Err(e) => {
                        error!("Failed to deregister agent {}: {}", agent_id, e);
                        Ok(Some(JsonRpcResponse::error(
                            request.id,
                            JsonRpcError {
                                code: error_codes::INTERNAL_ERROR,
                                message: format!("Agent deregistration failed: {}", e),
                                data: None,
                            },
                        )))
                    }
                }
            }
            Ok(None) => {
                warn!("Attempted to deregister non-existent agent: {}", agent_id);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Agent not found: {}", agent_id),
                        data: None,
                    },
                )))
            }
            Err(e) => {
                error!("Failed to check agent existence {}: {}", agent_id, e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to verify agent: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Format agent status for JSON response
    fn format_agent_status(&self, status: &AgentStatus) -> String {
        match status {
            AgentStatus::Connecting => "Connecting".to_string(),
            AgentStatus::Online => "Online".to_string(),
            AgentStatus::Idle => "Idle".to_string(),
            AgentStatus::Busy => "Busy".to_string(),
            AgentStatus::Maintenance => "Maintenance".to_string(),
            AgentStatus::Disconnecting => "Disconnecting".to_string(),
            AgentStatus::Offline => "Offline".to_string(),
            AgentStatus::Error { message } => format!("Error: {}", message),
            AgentStatus::Unhealthy { reason } => format!("Unhealthy: {}", reason),
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

    /// Handle message send request
    async fn handle_message_send(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling message send request");

        let message_service =
            self.message_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Message service not configured".to_string(),
                })?;

        // Parse request parameters
        #[derive(serde::Deserialize)]
        struct SendMessageParams {
            recipient_id: String,
            content: String,
            message_type: Option<String>,
            priority: Option<String>,
        }

        let params: SendMessageParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid message send parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing message send parameters".to_string(),
            });
        };

        // Parse recipient ID
        let recipient_id = Uuid::parse_str(&params.recipient_id).map_err(|e| Error::Protocol {
            message: format!("Invalid recipient ID: {}", e),
        })?;

        // Parse message type
        let message_type = match params.message_type.as_deref() {
            Some("Direct") => MessageType::Direct,
            Some("StatusUpdate") => MessageType::StatusUpdate,
            Some("IssueNotification") => MessageType::IssueNotification,
            Some("KnowledgeShare") => MessageType::KnowledgeShare,
            None => MessageType::Direct, // Default
            Some(t) => {
                return Err(Error::Protocol {
                    message: format!("Invalid message type: {}", t),
                });
            }
        };

        // Parse priority
        let priority = match params.priority.as_deref() {
            Some("Low") => MessagePriority::Low,
            Some("Normal") => MessagePriority::Normal,
            Some("High") => MessagePriority::High,
            Some("Urgent") => MessagePriority::Urgent,
            None => MessagePriority::Normal, // Default
            Some(p) => {
                return Err(Error::Protocol {
                    message: format!("Invalid priority: {}", p),
                });
            }
        };

        // Validate content
        if let Err(e) = message_service.validate_message_content(&params.content) {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Invalid message content: {}", e),
                    data: None,
                },
            )));
        }

        // TODO: Get sender ID from authenticated session
        let sender_id = Uuid::new_v4(); // Placeholder

        // Send the message
        match message_service
            .send_message(
                sender_id,
                recipient_id,
                params.content,
                message_type,
                priority,
            )
            .await
        {
            Ok(message) => {
                let result = serde_json::json!({
                    "message_id": message.id,
                    "sender_id": message.sender_id,
                    "recipient_id": message.recipient_id,
                    "content": message.content,
                    "message_type": format!("{:?}", message.message_type),
                    "priority": format!("{:?}", message.metadata.priority),
                    "created_at": message.created_at,
                    "status": "sent"
                });
                Ok(Some(JsonRpcResponse::success(request.id, result)))
            }
            Err(e) => {
                error!("Failed to send message: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to send message: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle message broadcast request
    async fn handle_message_broadcast(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling message broadcast request");

        let message_service =
            self.message_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Message service not configured".to_string(),
                })?;

        // Parse request parameters
        #[derive(serde::Deserialize)]
        struct BroadcastMessageParams {
            content: String,
            message_type: Option<String>,
            priority: Option<String>,
        }

        let params: BroadcastMessageParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid broadcast parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing broadcast parameters".to_string(),
            });
        };

        // Parse message type
        let message_type = match params.message_type.as_deref() {
            Some("Broadcast") => MessageType::Broadcast,
            Some("StatusUpdate") => MessageType::StatusUpdate,
            Some("IssueNotification") => MessageType::IssueNotification,
            Some("KnowledgeShare") => MessageType::KnowledgeShare,
            None => MessageType::Broadcast, // Default
            Some(t) => {
                return Err(Error::Protocol {
                    message: format!("Invalid message type: {}", t),
                });
            }
        };

        // Parse priority
        let priority = match params.priority.as_deref() {
            Some("Low") => MessagePriority::Low,
            Some("Normal") => MessagePriority::Normal,
            Some("High") => MessagePriority::High,
            Some("Urgent") => MessagePriority::Urgent,
            None => MessagePriority::Normal, // Default
            Some(p) => {
                return Err(Error::Protocol {
                    message: format!("Invalid priority: {}", p),
                });
            }
        };

        // Validate content
        if let Err(e) = message_service.validate_message_content(&params.content) {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Invalid message content: {}", e),
                    data: None,
                },
            )));
        }

        // TODO: Get sender ID from authenticated session
        let sender_id = Uuid::new_v4(); // Placeholder

        // Send the broadcast
        match message_service
            .send_broadcast(sender_id, params.content, message_type, priority)
            .await
        {
            Ok(message) => {
                let result = serde_json::json!({
                    "message_id": message.id,
                    "sender_id": message.sender_id,
                    "content": message.content,
                    "message_type": format!("{:?}", message.message_type),
                    "priority": format!("{:?}", message.metadata.priority),
                    "created_at": message.created_at,
                    "delivered_at": message.delivered_at,
                    "status": "broadcast"
                });
                Ok(Some(JsonRpcResponse::success(request.id, result)))
            }
            Err(e) => {
                error!("Failed to send broadcast: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to send broadcast: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle new issue creation with comprehensive parameters
    async fn handle_issue_create_new(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling new issue creation request");

        let issue_service = self
            .issue_service
            .as_ref()
            .ok_or_else(|| Error::Configuration {
                message: "Issue service not configured".to_string(),
            })?;

        // Parse request parameters
        let params: IssueCreateParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid issue creation parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing issue creation parameters".to_string(),
            });
        };

        info!(
            "Creating new issue: {} by agent {}",
            params.title, params.created_by_agent_id
        );

        // Validate created_by_agent_id
        let _created_by_agent_id =
            Uuid::parse_str(&params.created_by_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid agent ID: {}", e),
            })?;

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

        let tags = params.labels.unwrap_or_default();

        // Create the issue
        match issue_service
            .create_issue(params.title.clone(), params.description, priority, tags)
            .await
        {
            Ok(mut issue) => {
                // Handle assignment if provided
                if let Some(assignee) = params.assignee {
                    let assignee_id = Uuid::parse_str(&assignee).map_err(|e| Error::Protocol {
                        message: format!("Invalid assignee ID: {}", e),
                    })?;

                    // Assign the issue
                    if let Err(e) = issue_service.assign_issue(issue.id, assignee_id).await {
                        warn!(
                            "Failed to assign newly created issue {} to {}: {}",
                            issue.id, assignee_id, e
                        );
                    } else {
                        issue.assigned_agent_id = Some(assignee_id);
                        info!(
                            "Successfully assigned new issue {} to agent {}",
                            issue.id, assignee_id
                        );
                    }
                }

                let result = IssueCreateResult {
                    issue_id: issue.id,
                    title: issue.title,
                    status: format!("{:?}", issue.status),
                    priority: format!("{:?}", issue.priority),
                    created_at: issue.created_at,
                    message: "Issue created successfully".to_string(),
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
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

    /// Handle new issue list request with comprehensive filtering
    async fn handle_issue_list_new(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling new issue list request");

        let issue_service = self
            .issue_service
            .as_ref()
            .ok_or_else(|| Error::Configuration {
                message: "Issue service not configured".to_string(),
            })?;

        // Parse optional filter parameters
        let params: IssueListParams = if let Some(params) = request.params {
            serde_json::from_value(params).unwrap_or_default()
        } else {
            IssueListParams::default()
        };

        // Get issues based on filters
        let issues_result = if let Some(status_str) = &params.status {
            let status = match status_str.as_str() {
                "Open" => IssueStatus::Open,
                "InProgress" => IssueStatus::InProgress,
                "Resolved" => IssueStatus::Resolved,
                "Closed" => IssueStatus::Closed,
                s if s.starts_with("Blocked:") => {
                    let reason = s.strip_prefix("Blocked:").unwrap_or("").to_string();
                    IssueStatus::Blocked { reason }
                }
                _ => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid status filter: {}", status_str),
                            data: None,
                        },
                    )));
                }
            };
            issue_service.get_issues_by_status(&status).await
        } else if let Some(priority_str) = &params.priority {
            let priority = match priority_str.as_str() {
                "Low" => IssuePriority::Low,
                "Medium" => IssuePriority::Medium,
                "High" => IssuePriority::High,
                "Critical" => IssuePriority::Critical,
                _ => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid priority filter: {}", priority_str),
                            data: None,
                        },
                    )));
                }
            };
            issue_service.get_issues_by_priority(&priority).await
        } else if let Some(assignee_str) = &params.assignee {
            let assignee_id = Uuid::parse_str(assignee_str).map_err(|e| Error::Protocol {
                message: format!("Invalid assignee ID: {}", e),
            })?;
            issue_service.get_agent_issues(assignee_id).await
        } else {
            issue_service.list_issues().await
        };

        match issues_result {
            Ok(mut issues) => {
                // Apply limit if specified
                if let Some(limit) = params.limit {
                    issues.truncate(limit);
                }

                let issue_data: Vec<IssueInfo> = issues
                    .iter()
                    .map(|issue| IssueInfo {
                        id: issue.id,
                        title: issue.title.clone(),
                        description: issue.description.clone(),
                        priority: format!("{:?}", issue.priority),
                        status: match &issue.status {
                            IssueStatus::Blocked { reason } => format!("Blocked: {}", reason),
                            other => format!("{:?}", other),
                        },
                        assigned_agent_id: issue.assigned_agent_id,
                        created_at: issue.created_at,
                        updated_at: issue.updated_at,
                        resolved_at: issue.resolved_at,
                        tags: issue.tags.clone(),
                        knowledge_links: issue.knowledge_links.clone(),
                        is_assigned: issue.is_assigned(),
                        is_terminal: issue.is_terminal(),
                        age_seconds: issue.age_seconds(),
                    })
                    .collect();

                let result = IssueListResult {
                    issues: issue_data,
                    total: issues.len(),
                    filters_applied: params,
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
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

    /// Handle issue assignment request
    async fn handle_issue_assign(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue assignment request");

        let issue_service = self
            .issue_service
            .as_ref()
            .ok_or_else(|| Error::Configuration {
                message: "Issue service not configured".to_string(),
            })?;

        // Parse request parameters
        let params: IssueAssignParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid issue assignment parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing issue assignment parameters".to_string(),
            });
        };

        let issue_id = Uuid::parse_str(&params.issue_id).map_err(|e| Error::Protocol {
            message: format!("Invalid issue ID: {}", e),
        })?;

        let assignee_agent_id =
            Uuid::parse_str(&params.assignee_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid assignee agent ID: {}", e),
            })?;

        let assigned_by_agent_id =
            Uuid::parse_str(&params.assigned_by_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid assigned by agent ID: {}", e),
            })?;

        info!(
            "Assigning issue {} to agent {} by agent {}",
            issue_id, assignee_agent_id, assigned_by_agent_id
        );

        // Assign the issue
        match issue_service
            .assign_issue(issue_id, assignee_agent_id)
            .await
        {
            Ok(issue) => {
                let result = IssueAssignResult {
                    issue_id: issue.id,
                    assignee_agent_id,
                    assigned_by_agent_id,
                    status: format!("{:?}", issue.status),
                    assigned_at: issue.updated_at,
                    message: "Issue assigned successfully".to_string(),
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
            }
            Err(e) => {
                error!("Failed to assign issue {}: {}", issue_id, e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to assign issue: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle new issue update request with status and comment handling
    async fn handle_issue_update_new(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling new issue update request");

        let issue_service = self
            .issue_service
            .as_ref()
            .ok_or_else(|| Error::Configuration {
                message: "Issue service not configured".to_string(),
            })?;

        // Parse request parameters
        let params: IssueUpdateParams = if let Some(params) = request.params {
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

        let _updated_by_agent_id =
            Uuid::parse_str(&params.updated_by_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid updated by agent ID: {}", e),
            })?;

        info!(
            "Updating issue {} by agent {}",
            issue_id, _updated_by_agent_id
        );

        // Handle status update if provided
        let mut updated_issue = if let Some(status_str) = &params.status {
            let new_status = match status_str.as_str() {
                "Open" => IssueStatus::Open,
                "InProgress" => IssueStatus::InProgress,
                "Resolved" => IssueStatus::Resolved,
                "Closed" => IssueStatus::Closed,
                s if s.starts_with("Blocked:") => {
                    let reason = s.strip_prefix("Blocked:").unwrap_or("").to_string();
                    IssueStatus::Blocked { reason }
                }
                _ => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid status: {}", status_str),
                            data: None,
                        },
                    )));
                }
            };

            match issue_service.change_status(issue_id, new_status).await {
                Ok(issue) => issue,
                Err(e) => {
                    error!("Failed to update issue status: {}", e);
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to update issue status: {}", e),
                            data: None,
                        },
                    )));
                }
            }
        } else {
            // Get current issue for other updates
            match issue_service.get_issue(issue_id).await? {
                Some(issue) => issue,
                None => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Issue not found: {}", issue_id),
                            data: None,
                        },
                    )));
                }
            }
        };

        // Handle priority update if provided
        if let Some(priority_str) = &params.priority {
            let priority = match priority_str.as_str() {
                "Low" => IssuePriority::Low,
                "Medium" => IssuePriority::Medium,
                "High" => IssuePriority::High,
                "Critical" => IssuePriority::Critical,
                _ => {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Invalid priority: {}", priority_str),
                            data: None,
                        },
                    )));
                }
            };

            match issue_service.update_priority(issue_id, priority).await {
                Ok(issue) => updated_issue = issue,
                Err(e) => {
                    error!("Failed to update issue priority: {}", e);
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to update issue priority: {}", e),
                            data: None,
                        },
                    )));
                }
            }
        }

        // TODO: Handle comment addition - this would require extending the IssueService
        // For now, we just acknowledge the comment parameter
        let comment_added = params.comment.is_some();

        let result = IssueUpdateResult {
            issue_id: updated_issue.id,
            status: format!("{:?}", updated_issue.status),
            priority: Some(format!("{:?}", updated_issue.priority)),
            updated_at: updated_issue.updated_at,
            comment_added,
            message: "Issue updated successfully".to_string(),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle issue close request
    async fn handle_issue_close(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling issue close request");

        let issue_service = self
            .issue_service
            .as_ref()
            .ok_or_else(|| Error::Configuration {
                message: "Issue service not configured".to_string(),
            })?;

        // Parse request parameters
        let params: IssueCloseParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid issue close parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing issue close parameters".to_string(),
            });
        };

        let issue_id = Uuid::parse_str(&params.issue_id).map_err(|e| Error::Protocol {
            message: format!("Invalid issue ID: {}", e),
        })?;

        let closed_by_agent_id =
            Uuid::parse_str(&params.closed_by_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid closed by agent ID: {}", e),
            })?;

        info!(
            "Closing issue {} with resolution '{}' by agent {}",
            issue_id, params.resolution, closed_by_agent_id
        );

        // Close the issue
        match issue_service.close_issue(issue_id).await {
            Ok(issue) => {
                let result = IssueCloseResult {
                    issue_id: issue.id,
                    closed_by_agent_id,
                    status: format!("{:?}", issue.status),
                    resolution: params.resolution,
                    closed_at: issue.resolved_at.unwrap_or(issue.updated_at),
                    message: "Issue closed successfully".to_string(),
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
            }
            Err(e) => {
                error!("Failed to close issue {}: {}", issue_id, e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INTERNAL_ERROR,
                        message: format!("Failed to close issue: {}", e),
                        data: None,
                    },
                )))
            }
        }
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

    /// Handle worker message request - send direct messages between workers
    async fn handle_worker_message(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling worker message request");

        let message_service =
            self.message_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Message service not configured".to_string(),
                })?;

        // Parse request parameters
        let params: WorkerMessageParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid worker message parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing worker message parameters".to_string(),
            });
        };

        // Parse and validate agent IDs
        let sender_id = Uuid::parse_str(&params.sender_agent_id).map_err(|e| Error::Protocol {
            message: format!("Invalid sender agent ID: {}", e),
        })?;

        let recipient_id =
            Uuid::parse_str(&params.recipient_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid recipient agent ID: {}", e),
            })?;

        // Validate agents exist if agent service is available
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(sender_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Sender agent not found: {}", sender_id),
                        data: None,
                    },
                )));
            }

            if agent_service.get_agent(recipient_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Recipient agent not found: {}", recipient_id),
                        data: None,
                    },
                )));
            }
        }

        // Parse message type
        let message_type = match params.message_type.as_str() {
            "Info" => MessageType::Direct,
            "Request" => MessageType::Direct,
            "Coordination" => MessageType::StatusUpdate,
            "Alert" => MessageType::IssueNotification,
            _ => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INVALID_PARAMS,
                        message: format!("Invalid message type: {}", params.message_type),
                        data: None,
                    },
                )));
            }
        };

        // Parse priority
        let priority = match params.priority.as_deref() {
            Some("Low") => MessagePriority::Low,
            Some("Normal") => MessagePriority::Normal,
            Some("High") => MessagePriority::High,
            Some("Urgent") => MessagePriority::Urgent,
            None => MessagePriority::Normal,
            Some(p) => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INVALID_PARAMS,
                        message: format!("Invalid priority: {}", p),
                        data: None,
                    },
                )));
            }
        };

        // Validate message content
        if let Err(e) = message_service.validate_message_content(&params.message_content) {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Invalid message content: {}", e),
                    data: None,
                },
            )));
        }

        info!(
            "Sending worker message from {} to {}: {}",
            sender_id, recipient_id, params.message_type
        );

        // Send the message
        match message_service
            .send_message(
                sender_id,
                recipient_id,
                params.message_content,
                message_type,
                priority,
            )
            .await
        {
            Ok(message) => {
                let result = WorkerMessageResult {
                    message_id: message.id,
                    recipient_agent_id: recipient_id,
                    sender_agent_id: sender_id,
                    status: "sent".to_string(),
                    sent_at: message.created_at,
                    delivery_confirmation: message.requires_confirmation(),
                    message: "Worker message sent successfully".to_string(),
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
            }
            Err(e) => {
                error!("Failed to send worker message: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::MESSAGE_DELIVERY_FAILED,
                        message: format!("Failed to send worker message: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle worker request - request specific actions from targeted workers
    async fn handle_worker_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling worker request");

        let message_service =
            self.message_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Message service not configured".to_string(),
                })?;

        // Parse request parameters
        let params: WorkerRequestParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid worker request parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing worker request parameters".to_string(),
            });
        };

        // Parse and validate agent IDs
        let requester_id =
            Uuid::parse_str(&params.requested_by_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid requester agent ID: {}", e),
            })?;

        let target_id = Uuid::parse_str(&params.target_agent_id).map_err(|e| Error::Protocol {
            message: format!("Invalid target agent ID: {}", e),
        })?;

        // Validate agents exist if agent service is available
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(requester_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Requester agent not found: {}", requester_id),
                        data: None,
                    },
                )));
            }

            if agent_service.get_agent(target_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Target agent not found: {}", target_id),
                        data: None,
                    },
                )));
            }
        }

        // Parse priority
        let priority = match params.priority.as_deref() {
            Some("Low") => MessagePriority::Low,
            Some("Normal") => MessagePriority::Normal,
            Some("High") => MessagePriority::High,
            Some("Urgent") => MessagePriority::Urgent,
            None => MessagePriority::High, // Requests default to high priority
            Some(p) => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INVALID_PARAMS,
                        message: format!("Invalid priority: {}", p),
                        data: None,
                    },
                )));
            }
        };

        // Create request message content
        let request_content = format!(
            "ACTION REQUEST: {}\n\nDetails: {}\nRequested by: {}{}",
            params.request_type,
            serde_json::to_string_pretty(&params.request_details)
                .unwrap_or_else(|_| "Unable to serialize request details".to_string()),
            requester_id,
            params
                .deadline
                .map(|d| format!("\nDeadline: {}", d.format("%Y-%m-%d %H:%M:%S UTC")))
                .unwrap_or_default()
        );

        info!(
            "Creating worker request from {} to {}: {}",
            requester_id, target_id, params.request_type
        );

        // Send as a direct message with request type
        match message_service
            .send_message(
                requester_id,
                target_id,
                request_content,
                MessageType::Direct,
                priority,
            )
            .await
        {
            Ok(message) => {
                let result = WorkerRequestResult {
                    request_id: message.id,
                    target_agent_id: target_id,
                    requested_by_agent_id: requester_id,
                    request_type: params.request_type,
                    status: "sent".to_string(),
                    created_at: message.created_at,
                    deadline: params.deadline,
                    message: "Worker request sent successfully".to_string(),
                };

                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(result)?,
                )))
            }
            Err(e) => {
                error!("Failed to send worker request: {}", e);
                Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::MESSAGE_DELIVERY_FAILED,
                        message: format!("Failed to send worker request: {}", e),
                        data: None,
                    },
                )))
            }
        }
    }

    /// Handle worker coordination - coordinate overlapping work areas between multiple workers
    async fn handle_worker_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling worker coordination request");

        let message_service =
            self.message_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Message service not configured".to_string(),
                })?;

        // Parse request parameters
        let params: WorkerCoordinateParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid worker coordination parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing worker coordination parameters".to_string(),
            });
        };

        // Parse and validate coordinator agent ID
        let coordinator_id =
            Uuid::parse_str(&params.coordinator_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid coordinator agent ID: {}", e),
            })?;

        // Parse and validate involved agent IDs
        let mut involved_agent_ids = Vec::new();
        for agent_id_str in &params.involved_agents {
            let agent_id = Uuid::parse_str(agent_id_str).map_err(|e| Error::Protocol {
                message: format!("Invalid involved agent ID '{}': {}", agent_id_str, e),
            })?;
            involved_agent_ids.push(agent_id);
        }

        if involved_agent_ids.is_empty() {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "At least one involved agent is required".to_string(),
                    data: None,
                },
            )));
        }

        // Validate agents exist if agent service is available
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(coordinator_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Coordinator agent not found: {}", coordinator_id),
                        data: None,
                    },
                )));
            }

            for agent_id in &involved_agent_ids {
                if agent_service.get_agent(*agent_id).await?.is_none() {
                    return Ok(Some(JsonRpcResponse::error(
                        request.id,
                        JsonRpcError {
                            code: error_codes::AGENT_NOT_FOUND,
                            message: format!("Involved agent not found: {}", agent_id),
                            data: None,
                        },
                    )));
                }
            }
        }

        // Generate coordination session ID
        let coordination_session_id = Uuid::new_v4();

        // Create coordination message content
        let coordination_content = format!(
            "COORDINATION SESSION: {}\n\nType: {}\nSession ID: {}\nCoordinator: {}\nScope: {}\nDetails: {}\n\nPlease acknowledge participation in this coordination session.",
            params.coordination_type,
            params.coordination_type,
            coordination_session_id,
            coordinator_id,
            serde_json::to_string_pretty(&params.scope)
                .unwrap_or_else(|_| "Unable to serialize scope".to_string()),
            serde_json::to_string_pretty(&params.details)
                .unwrap_or_else(|_| "Unable to serialize details".to_string())
        );

        info!(
            "Creating coordination session {} with {} participants",
            coordination_session_id,
            involved_agent_ids.len()
        );

        // Send coordination messages to all involved agents
        let mut participant_confirmations = Vec::new();
        for agent_id in &involved_agent_ids {
            match message_service
                .send_message(
                    coordinator_id,
                    *agent_id,
                    coordination_content.clone(),
                    MessageType::StatusUpdate,
                    MessagePriority::High,
                )
                .await
            {
                Ok(_) => {
                    participant_confirmations.push(format!("Sent to {}", agent_id));
                }
                Err(e) => {
                    warn!("Failed to send coordination message to {}: {}", agent_id, e);
                    participant_confirmations
                        .push(format!("Failed to send to {}: {}", agent_id, e));
                }
            }
        }

        let result = WorkerCoordinateResult {
            coordination_session_id,
            coordinator_agent_id: coordinator_id,
            involved_agents: involved_agent_ids,
            coordination_type: params.coordination_type,
            status: "initiated".to_string(),
            created_at: chrono::Utc::now(),
            participant_confirmations,
            message: "Coordination session initiated successfully".to_string(),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle project lock - create project-level coordination locks to prevent conflicts
    async fn handle_project_lock(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling project lock request");

        // Parse request parameters
        let params: ProjectLockParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid project lock parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing project lock parameters".to_string(),
            });
        };

        // Parse and validate lock holder agent ID
        let lock_holder_id =
            Uuid::parse_str(&params.lock_holder_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid lock holder agent ID: {}", e),
            })?;

        // Validate agent exists if agent service is available
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(lock_holder_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Lock holder agent not found: {}", lock_holder_id),
                        data: None,
                    },
                )));
            }
        }

        // Validate lock type
        if !["Exclusive", "Shared", "Coordination"].contains(&params.lock_type.as_str()) {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Invalid lock type: {}", params.lock_type),
                    data: None,
                },
            )));
        }

        // Generate lock ID
        let lock_id = Uuid::new_v4();

        // Calculate expiration time if duration is provided
        let expiration = params
            .duration
            .map(|duration| chrono::Utc::now() + chrono::Duration::seconds(duration));

        // For now, we'll simulate successful lock acquisition
        // In a real implementation, this would involve checking for existing locks
        // and managing a distributed lock registry

        let locked_at = chrono::Utc::now();

        info!(
            "Creating {} lock {} for resource '{}' by agent {}",
            params.lock_type, lock_id, params.resource_path, lock_holder_id
        );

        // Send notification to relevant agents if message service is available
        if let Some(message_service) = &self.message_service {
            let lock_notification = format!(
                "RESOURCE LOCK ACQUIRED\n\nLock ID: {}\nResource: {}\nLock Type: {}\nHolder: {}\nReason: {}\nExpires: {}",
                lock_id,
                params.resource_path,
                params.lock_type,
                lock_holder_id,
                params.reason,
                expiration.map(|e| e.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or("Never".to_string())
            );

            // Send as broadcast to notify all agents about the lock
            if let Err(e) = message_service
                .send_broadcast(
                    lock_holder_id,
                    lock_notification,
                    MessageType::StatusUpdate,
                    MessagePriority::Normal,
                )
                .await
            {
                warn!("Failed to broadcast lock notification: {}", e);
            }
        }

        let result = ProjectLockResult {
            lock_id,
            project_id: params.project_id,
            resource_path: params.resource_path,
            lock_type: params.lock_type,
            lock_holder_agent_id: lock_holder_id,
            status: "acquired".to_string(),
            locked_at,
            expiration,
            message: "Project lock acquired successfully".to_string(),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    // Cross-project dependency coordination handlers

    /// Handle dependency declaration - declare cross-project dependency and create coordination plan
    async fn handle_dependency_declare(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling dependency declare request");

        let coordination_service = self
            .coordination_service
            .as_ref()
            .ok_or_else(|| Error::service_unavailable("Coordination service not available"))?;

        let params: DependencyDeclareParams =
            serde_json::from_value(request.params.unwrap_or(serde_json::Value::Null))?;

        // Parse UUID from string
        let declaring_agent_id = Uuid::parse_str(&params.declaring_agent_id)
            .map_err(|_| Error::validation("Invalid declaring_agent_id UUID"))?;

        // Parse dependency type
        let dependency_type = match params.dependency_type.as_str() {
            "API_CHANGE" => vibe_ensemble_core::coordination::DependencyType::ApiChange,
            "SHARED_RESOURCE" => vibe_ensemble_core::coordination::DependencyType::SharedResource,
            "BUILD_DEPENDENCY" => vibe_ensemble_core::coordination::DependencyType::BuildDependency,
            "CONFIGURATION" => vibe_ensemble_core::coordination::DependencyType::Configuration,
            "DATA_SCHEMA" => vibe_ensemble_core::coordination::DependencyType::DataSchema,
            custom => vibe_ensemble_core::coordination::DependencyType::Custom(custom.to_string()),
        };

        // Parse impact level
        let impact = match params.impact.as_str() {
            "BLOCKER" => vibe_ensemble_core::coordination::DependencyImpact::Blocker,
            "MAJOR" => vibe_ensemble_core::coordination::DependencyImpact::Major,
            "MINOR" => vibe_ensemble_core::coordination::DependencyImpact::Minor,
            "INFO" => vibe_ensemble_core::coordination::DependencyImpact::Info,
            _ => vibe_ensemble_core::coordination::DependencyImpact::Major,
        };

        // Parse urgency level
        let urgency = match params.urgency.as_str() {
            "CRITICAL" => vibe_ensemble_core::coordination::DependencyUrgency::Critical,
            "HIGH" => vibe_ensemble_core::coordination::DependencyUrgency::High,
            "MEDIUM" => vibe_ensemble_core::coordination::DependencyUrgency::Medium,
            "LOW" => vibe_ensemble_core::coordination::DependencyUrgency::Low,
            _ => vibe_ensemble_core::coordination::DependencyUrgency::Medium,
        };

        // Parse metadata
        let metadata = if let Some(metadata_value) = params.metadata {
            serde_json::from_value::<std::collections::HashMap<String, String>>(metadata_value)
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        // Declare dependency
        let (dependency, coordination_plan, issue) = coordination_service
            .declare_dependency(
                declaring_agent_id,
                params.source_project,
                params.target_project,
                dependency_type,
                params.description,
                impact,
                urgency,
                params.affected_files,
                metadata,
            )
            .await?;

        info!(
            "Dependency declared: {} -> {} (ID: {})",
            dependency.source_project, dependency.target_project, dependency.id
        );

        // Build response
        let result = DependencyDeclareResult {
            dependency_id: dependency.id,
            coordination_plan: serde_json::to_value(&coordination_plan)?,
            required_actions: coordination_plan
                .required_actions
                .iter()
                .map(|action| serde_json::to_value(action).unwrap_or(serde_json::Value::Null))
                .collect(),
            target_project_active_workers: coordination_plan.assigned_agents,
            issue_created: issue.map(|i| i.id),
            status: format!("{:?}", dependency.status),
            estimated_resolution_time: coordination_plan
                .estimated_duration
                .map(|d| chrono::Utc::now() + d),
            message: format!(
                "Dependency declared successfully with {} coordination plan",
                match coordination_plan.plan_type {
                    vibe_ensemble_core::coordination::CoordinationPlanType::DirectCoordination =>
                        "direct",
                    vibe_ensemble_core::coordination::CoordinationPlanType::WorkerSpawn =>
                        "worker spawn",
                    _ => "custom",
                }
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle coordinator worker request - request coordinator spawn new worker
    async fn handle_coordinator_request_worker(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling coordinator request worker");

        let coordination_service = self
            .coordination_service
            .as_ref()
            .ok_or_else(|| Error::service_unavailable("Coordination service not available"))?;

        let params: CoordinatorRequestWorkerParams =
            serde_json::from_value(request.params.unwrap_or(serde_json::Value::Null))?;

        // Parse UUID from string
        let requesting_agent_id = Uuid::parse_str(&params.requesting_agent_id)
            .map_err(|_| Error::validation("Invalid requesting_agent_id UUID"))?;

        // Parse priority
        let priority = match params.priority.as_str() {
            "CRITICAL" => vibe_ensemble_core::coordination::SpawnPriority::Critical,
            "HIGH" => vibe_ensemble_core::coordination::SpawnPriority::High,
            "MEDIUM" => vibe_ensemble_core::coordination::SpawnPriority::Medium,
            "LOW" => vibe_ensemble_core::coordination::SpawnPriority::Low,
            _ => vibe_ensemble_core::coordination::SpawnPriority::Medium,
        };

        // Parse estimated duration
        let estimated_duration = params.estimated_duration.and_then(|duration_str| {
            // Try to parse duration string like "2h", "30m", "1h30m"
            if let Ok(minutes) = duration_str.trim_end_matches("m").parse::<i64>() {
                Some(chrono::Duration::minutes(minutes))
            } else if let Ok(hours) = duration_str.trim_end_matches("h").parse::<i64>() {
                Some(chrono::Duration::hours(hours))
            } else {
                None
            }
        });

        // Parse context data
        let context_data = if let Some(context_value) = params.context_data {
            serde_json::from_value::<std::collections::HashMap<String, String>>(context_value)
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        // Request worker spawn
        let spawn_request = coordination_service
            .request_worker_spawn(
                requesting_agent_id,
                params.target_project,
                params.required_capabilities.clone(),
                priority,
                params.task_description,
                estimated_duration,
                context_data,
            )
            .await?;

        info!(
            "Worker spawn requested for project {} (ID: {})",
            spawn_request.target_project, spawn_request.id
        );

        // Build response
        let result = CoordinatorRequestWorkerResult {
            request_id: spawn_request.id,
            worker_assignment_status: format!("{:?}", spawn_request.status),
            estimated_spawn_time: spawn_request
                .estimated_duration
                .map(|d| chrono::Utc::now() + d),
            assigned_worker_id: spawn_request.assigned_worker_id,
            capability_match: 1.0, // TODO: Calculate actual capability match score
            spawn_plan: spawn_request
                .spawn_result
                .as_ref()
                .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null)),
            status: format!("{:?}", spawn_request.status),
            message: match spawn_request.status {
                vibe_ensemble_core::coordination::SpawnRequestStatus::Approved => {
                    "Worker spawn request approved and processing"
                }
                vibe_ensemble_core::coordination::SpawnRequestStatus::Evaluating => {
                    "Worker spawn request under evaluation"
                }
                vibe_ensemble_core::coordination::SpawnRequestStatus::Pending => {
                    "Worker spawn request queued for processing"
                }
                _ => "Worker spawn request submitted",
            }
            .to_string(),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle work coordination - negotiate work ordering between workers
    async fn handle_work_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling work coordination request");

        let coordination_service = self
            .coordination_service
            .as_ref()
            .ok_or_else(|| Error::service_unavailable("Coordination service not available"))?;

        let params: WorkCoordinateParams =
            serde_json::from_value(request.params.unwrap_or(serde_json::Value::Null))?;

        // Parse UUIDs from strings
        let initiating_agent_id = Uuid::parse_str(&params.initiating_agent_id)
            .map_err(|_| Error::validation("Invalid initiating_agent_id UUID"))?;

        let target_agent_id = Uuid::parse_str(&params.target_agent_id)
            .map_err(|_| Error::validation("Invalid target_agent_id UUID"))?;

        // Parse coordination type
        let coordination_type = match params.coordination_type.as_str() {
            "SEQUENTIAL" => vibe_ensemble_core::coordination::WorkCoordinationType::Sequential,
            "PARALLEL" => vibe_ensemble_core::coordination::WorkCoordinationType::Parallel,
            "BLOCKING" => vibe_ensemble_core::coordination::WorkCoordinationType::Blocking,
            "COLLABORATIVE" => {
                vibe_ensemble_core::coordination::WorkCoordinationType::Collaborative
            }
            "CONFLICT_RESOLUTION" => {
                vibe_ensemble_core::coordination::WorkCoordinationType::ConflictResolution
            }
            _ => vibe_ensemble_core::coordination::WorkCoordinationType::Sequential,
        };

        // Parse work items and dependencies from JSON values
        let work_items: Vec<vibe_ensemble_core::coordination::WorkItem> = params
            .work_items
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect();

        let dependencies: Vec<vibe_ensemble_core::coordination::WorkDependency> = params
            .dependencies
            .into_iter()
            .filter_map(|dep| serde_json::from_value(dep).ok())
            .collect();

        let proposed_timeline: Option<vibe_ensemble_core::coordination::CoordinationTimeline> =
            params
                .proposed_timeline
                .and_then(|timeline| serde_json::from_value(timeline).ok());

        // Coordinate work
        let agreement = coordination_service
            .coordinate_work(
                initiating_agent_id,
                target_agent_id,
                coordination_type,
                work_items,
                dependencies,
                proposed_timeline,
            )
            .await?;

        info!(
            "Work coordination agreement created between agents {} and {} (ID: {})",
            agreement.initiating_agent_id, agreement.target_agent_id, agreement.id
        );

        // Build response
        let result = WorkCoordinateResult {
            coordination_agreement_id: agreement.id,
            negotiated_timeline: serde_json::to_value(&agreement.negotiated_timeline)?,
            work_assignments: agreement
                .work_items
                .iter()
                .map(|item| serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
                .collect(),
            coordination_status: format!("{:?}", agreement.status),
            participant_confirmations: vec![
                agreement.initiating_agent_id,
                agreement.target_agent_id,
            ],
            communication_protocol: serde_json::to_value(&agreement.terms.communication_protocol)?,
            escalation_rules: agreement
                .terms
                .escalation_rules
                .iter()
                .map(|rule| serde_json::to_value(rule).unwrap_or(serde_json::Value::Null))
                .collect(),
            message: format!(
                "{:?} coordination agreement established with {} work items",
                agreement.coordination_type,
                agreement.work_items.len()
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle conflict resolution - resolve overlapping modifications
    async fn handle_conflict_resolve(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling conflict resolution request");

        let coordination_service = self
            .coordination_service
            .as_ref()
            .ok_or_else(|| Error::service_unavailable("Coordination service not available"))?;

        let params: ConflictResolveParams =
            serde_json::from_value(request.params.unwrap_or(serde_json::Value::Null))?;

        // Parse UUIDs from strings
        let mut affected_agents = Vec::new();
        for id in &params.affected_agents {
            let uuid = Uuid::parse_str(id)
                .map_err(|_| Error::validation("Invalid affected_agents UUID format"))?;
            affected_agents.push(uuid);
        }

        let resolver_agent_id = Uuid::parse_str(&params.resolver_agent_id)
            .map_err(|_| Error::validation("Invalid resolver_agent_id UUID"))?;

        // Parse conflict type
        let conflict_type = match params.conflict_type.as_str() {
            "FILE_MODIFICATION" => vibe_ensemble_core::coordination::ConflictType::FileModification,
            "RESOURCE_LOCK" => vibe_ensemble_core::coordination::ConflictType::ResourceLock,
            "ARCHITECTURE" => vibe_ensemble_core::coordination::ConflictType::Architecture,
            "BUSINESS_LOGIC" => vibe_ensemble_core::coordination::ConflictType::BusinessLogic,
            "TESTING" => vibe_ensemble_core::coordination::ConflictType::Testing,
            "DEPLOYMENT" => vibe_ensemble_core::coordination::ConflictType::Deployment,
            _ => vibe_ensemble_core::coordination::ConflictType::FileModification,
        };

        // Parse resolution strategy
        let resolution_strategy =
            params
                .resolution_strategy
                .and_then(|strategy| match strategy.as_str() {
                    "LAST_WRITER_WINS" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::LastWriterWins)
                    }
                    "FIRST_WRITER_WINS" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::FirstWriterWins)
                    }
                    "AUTO_MERGE" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::AutoMerge)
                    }
                    "MANUAL_MERGE" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::ManualMerge)
                    }
                    "RESOURCE_SPLIT" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::ResourceSplit)
                    }
                    "SEQUENTIAL" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::Sequential)
                    }
                    "ESCALATE" => {
                        Some(vibe_ensemble_core::coordination::ResolutionStrategy::Escalate)
                    }
                    _ => None,
                });

        // Resolve conflict
        let conflict_case = coordination_service
            .resolve_conflict(
                affected_agents,
                params.conflicted_resources,
                conflict_type,
                resolution_strategy,
                resolver_agent_id,
            )
            .await?;

        info!(
            "Conflict resolution case created for {} agents (ID: {})",
            conflict_case.affected_agents.len(),
            conflict_case.id
        );

        // Build response
        let result = ConflictResolveResult {
            resolution_id: conflict_case.id,
            resolution_plan: conflict_case
                .resolution_plan
                .as_ref()
                .map(|plan| serde_json::to_value(plan).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null),
            required_actions_per_agent: conflict_case
                .resolution_plan
                .as_ref()
                .map(|plan| {
                    serde_json::to_value(&plan.required_actions_per_agent)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
                })
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            resolution_strategy: conflict_case
                .resolution_strategy
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "None".to_string()),
            estimated_resolution_time: conflict_case
                .resolution_plan
                .as_ref()
                .and_then(|plan| plan.estimated_resolution_time)
                .map(|d| chrono::Utc::now() + d),
            rollback_plan: conflict_case
                .resolution_plan
                .as_ref()
                .and_then(|plan| plan.rollback_plan.as_ref())
                .map(|rollback| serde_json::to_value(rollback).unwrap_or(serde_json::Value::Null)),
            coordinator_escalation: matches!(
                conflict_case.resolution_strategy,
                Some(vibe_ensemble_core::coordination::ResolutionStrategy::Escalate)
            ),
            status: format!("{:?}", conflict_case.status),
            message: format!(
                "{:?} conflict resolution case created with {} strategy",
                conflict_case.conflict_type,
                conflict_case
                    .resolution_strategy
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "automatic".to_string())
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
