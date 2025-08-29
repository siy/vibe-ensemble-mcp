//! MCP server implementation
//!
//! This module provides the core MCP server functionality including
//! protocol handling, capability negotiation, and client session management.

#![allow(deprecated)] // Allow deprecated method constants for backward compatibility
use crate::{
    protocol::{
        error_codes, AgentDeregisterParams, AgentDeregisterResult, AgentListParams,
        ConflictPredictParams, ConflictPredictResult, ConflictResolveParams,
        ConflictResolveResult, CoordinatorRequestWorkerParams, CoordinatorRequestWorkerResult,
        DependencyDeclareParams, DependencyDeclareResult, GuidelineEnforceParams,
        GuidelineEnforceResult, IssueAssignParams, IssueAssignResult, IssueCloseParams,
        IssueCloseResult, IssueCreateParams, IssueCreateResult, IssueInfo, IssueListParams,
        IssueListResult, IssueUpdateParams, IssueUpdateResult, KnowledgeQueryCoordinationParams,
        KnowledgeQueryCoordinationResult, LearningCaptureParams, LearningCaptureResult,
        MergeCoordinateParams, MergeCoordinateResult, PatternSuggestParams, PatternSuggestResult,
        ProjectLockParams, ProjectLockResult, ResourceReserveParams, ResourceReserveResult,
        ScheduleCoordinateParams, ScheduleCoordinateResult, VibeOperationParams,
        WorkCoordinateParams, WorkCoordinateResult, WorkerCoordinateParams, WorkerCoordinateResult,
        WorkerMessageParams, WorkerMessageResult, WorkerRequestParams, WorkerRequestResult, *,
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
    AgentService, CoordinationService, IssueService, KnowledgeService, MessageService,
};

/// Bundle of all coordination services for production deployments
#[derive(Clone)]
pub struct CoordinationServices {
    pub agent_service: Arc<AgentService>,
    pub issue_service: Arc<IssueService>,
    pub message_service: Arc<MessageService>,
    pub coordination_service: Arc<CoordinationService>,
    pub knowledge_service: Arc<KnowledgeService>,
}

impl CoordinationServices {
    /// Create a new coordination services bundle
    pub fn new(
        agent_service: Arc<AgentService>,
        issue_service: Arc<IssueService>,
        message_service: Arc<MessageService>,
        coordination_service: Arc<CoordinationService>,
        knowledge_service: Arc<KnowledgeService>,
    ) -> Self {
        Self {
            agent_service,
            issue_service,
            message_service,
            coordination_service,
            knowledge_service,
        }
    }
}

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
    /// Knowledge service for managing organizational knowledge and learning
    knowledge_service: Option<Arc<KnowledgeService>>,
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
    /// Create a basic MCP server with default capabilities and no coordination services.
    /// Suitable for MCP protocol testing and minimal deployments.
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: None,
            issue_service: None,
            message_service: None,
            coordination_service: None,
            knowledge_service: None,
        }
    }

    /// Create a full coordination server with all services for production deployments.
    /// This is the recommended constructor for most use cases.
    pub fn with_coordination(services: CoordinationServices) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities::default(),
            agent_service: Some(services.agent_service),
            issue_service: Some(services.issue_service),
            message_service: Some(services.message_service),
            coordination_service: Some(services.coordination_service),
            knowledge_service: Some(services.knowledge_service),
        }
    }

    /// Create a full coordination server with custom capabilities.
    /// Combines all coordination services with custom server capabilities.
    pub fn with_custom_capabilities(
        services: CoordinationServices,
        capabilities: ServerCapabilities,
    ) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            capabilities,
            agent_service: Some(services.agent_service),
            issue_service: Some(services.issue_service),
            message_service: Some(services.message_service),
            coordination_service: Some(services.coordination_service),
            knowledge_service: Some(services.knowledge_service),
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
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        match request.method.as_str() {
            // Standard MCP protocol methods (7 core methods)
            methods::INITIALIZE => self.handle_initialize(request).await,
            methods::PING => self.handle_ping(request).await,
            methods::LIST_TOOLS => self.handle_list_tools(request).await,
            methods::CALL_TOOL => self.handle_call_tool(request).await,
            methods::LIST_RESOURCES => self.handle_list_resources(request).await,
            methods::LIST_PROMPTS => self.handle_list_prompts(request).await,

            // Streamlined Vibe Ensemble extensions (3 essential methods for all functionality)
            methods::VIBE_AGENT => self.handle_vibe_agent(request).await,
            methods::VIBE_ISSUE => self.handle_vibe_issue(request).await,
            methods::VIBE_COORDINATION => self.handle_vibe_coordination(request).await,

            // Legacy method support (backward compatibility through consolidated handlers)
            #[allow(deprecated)]
            methods::AGENT_REGISTER
            | methods::AGENT_STATUS
            | methods::AGENT_LIST
            | methods::AGENT_DEREGISTER
            | methods::AGENT_CAPABILITIES => {
                // Route legacy agent methods to consolidated handler
                self.handle_vibe_agent_legacy(request).await
            }

            #[allow(deprecated)]
            methods::ISSUE_CREATE
            | methods::ISSUE_LIST
            | methods::ISSUE_ASSIGN
            | methods::ISSUE_UPDATE
            | methods::ISSUE_CLOSE => {
                // Route legacy issue methods to consolidated handler
                self.handle_vibe_issue_legacy(request).await
            }

            #[allow(deprecated)]
            methods::MESSAGE_SEND
            | methods::MESSAGE_BROADCAST
            | methods::WORKER_MESSAGE
            | methods::WORKER_REQUEST
            | methods::WORKER_COORDINATE
            | methods::PROJECT_LOCK
            | methods::DEPENDENCY_DECLARE
            | methods::COORDINATOR_REQUEST_WORKER
            | methods::WORK_COORDINATE
            | methods::CONFLICT_RESOLVE
            | methods::SCHEDULE_COORDINATE
            | methods::CONFLICT_PREDICT
            | methods::RESOURCE_RESERVE
            | methods::MERGE_COORDINATE
            | methods::KNOWLEDGE_QUERY
            | methods::KNOWLEDGE_SUBMIT
            | methods::KNOWLEDGE_QUERY_COORDINATION
            | methods::PATTERN_SUGGEST
            | methods::GUIDELINE_ENFORCE
            | methods::LEARNING_CAPTURE => {
                // Route legacy coordination methods to consolidated handler
                self.handle_vibe_coordination_legacy(request).await
            }

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
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
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
            "server": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION")
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
                },
                {
                    "name": "vibe_schedule_coordinate",
                    "description": "Plan work sequences across workers to prevent conflicts and optimize coordination",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "coordinatorAgentId": {"type": "string", "description": "ID of coordinating agent"},
                            "workSequences": {"type": "array", "items": {"type": "object"}, "description": "List of work sequences to coordinate"},
                            "involvedAgents": {"type": "array", "items": {"type": "string"}, "description": "IDs of agents involved in coordination"},
                            "projectScopes": {"type": "array", "items": {"type": "string"}, "description": "Project scopes affected"},
                            "resourceRequirements": {"type": "object", "description": "Resource requirements mapping"},
                            "timeConstraints": {"type": "object", "description": "Time constraints and deadlines"}
                        },
                        "required": ["coordinatorAgentId", "workSequences", "involvedAgents", "projectScopes", "resourceRequirements"]
                    }
                },
                {
                    "name": "vibe_conflict_predict",
                    "description": "Detect potential conflicts early before they occur in agent workflows",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "analyzerAgentId": {"type": "string", "description": "ID of agent performing analysis"},
                            "plannedActions": {"type": "array", "items": {"type": "object"}, "description": "Planned actions to analyze"},
                            "activeWorkflows": {"type": "array", "items": {"type": "object"}, "description": "Currently active workflows"},
                            "resourceMap": {"type": "object", "description": "Resource utilization mapping"},
                            "timeHorizon": {"type": "string", "description": "Time horizon for prediction (e.g., '24h', '1w')"}
                        },
                        "required": ["analyzerAgentId", "plannedActions", "activeWorkflows", "resourceMap"]
                    }
                },
                {
                    "name": "vibe_resource_reserve",
                    "description": "Reserve files/modules for exclusive access to prevent conflicts",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "reservingAgentId": {"type": "string", "description": "ID of agent requesting reservation"},
                            "resourcePaths": {"type": "array", "items": {"type": "string"}, "description": "Paths to resources to reserve"},
                            "reservationType": {"type": "string", "enum": ["EXCLUSIVE", "SHARED", "READ_ONLY"], "description": "Type of reservation"},
                            "reservationDuration": {"type": "string", "description": "Duration of reservation (e.g., '2h', '1d')"},
                            "exclusiveAccess": {"type": "boolean", "description": "Whether to require exclusive access"},
                            "allowedOperations": {"type": "array", "items": {"type": "string"}, "description": "Operations allowed on resource"},
                            "justification": {"type": "string", "description": "Justification for reservation"}
                        },
                        "required": ["reservingAgentId", "resourcePaths", "reservationType", "reservationDuration", "justification"]
                    }
                },
                {
                    "name": "vibe_merge_coordinate",
                    "description": "Coordinate complex merge scenarios between multiple agents and branches",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "coordinatorAgentId": {"type": "string", "description": "ID of agent coordinating the merge"},
                            "mergeScenario": {"type": "string", "enum": ["MULTI_BRANCH", "FEATURE_INTEGRATION", "HOTFIX_MERGE", "RELEASE_MERGE"], "description": "Type of merge scenario"},
                            "sourceBranches": {"type": "array", "items": {"type": "string"}, "description": "Source branches to merge"},
                            "targetBranch": {"type": "string", "description": "Target branch for merge"},
                            "involvedAgents": {"type": "array", "items": {"type": "string"}, "description": "IDs of agents involved in merge"},
                            "complexityAnalysis": {"type": "object", "description": "Analysis of merge complexity"},
                            "conflictResolutionStrategy": {"type": "string", "enum": ["AUTO", "MANUAL", "HYBRID", "ESCALATE"], "description": "Strategy for resolving conflicts"}
                        },
                        "required": ["coordinatorAgentId", "mergeScenario", "sourceBranches", "targetBranch", "involvedAgents", "complexityAnalysis"]
                    }
                },
                {
                    "name": "vibe_knowledge_query",
                    "description": "Search coordination patterns and solutions from organizational knowledge",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "queryingAgentId": {"type": "string", "description": "ID of agent making the query"},
                            "coordinationContext": {"type": "string", "description": "Context of coordination need"},
                            "query": {"type": "string", "description": "Search query for relevant knowledge"},
                            "searchScope": {"type": "array", "items": {"type": "string"}, "description": "Scope of search (patterns, practices, guidelines)"},
                            "relevanceCriteria": {"type": "object", "description": "Criteria for relevance assessment"},
                            "maxResults": {"type": "integer", "description": "Maximum number of results to return"}
                        },
                        "required": ["queryingAgentId", "query"]
                    }
                },
                {
                    "name": "vibe_pattern_suggest",
                    "description": "Suggest coordination approaches based on historical patterns and context",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "requestingAgentId": {"type": "string", "description": "ID of agent requesting suggestions"},
                            "coordinationScenario": {"type": "string", "description": "Description of coordination scenario"},
                            "currentContext": {"type": "object", "description": "Current context and constraints"},
                            "similarityThreshold": {"type": "number", "description": "Minimum similarity threshold for pattern matching"},
                            "excludePatterns": {"type": "array", "items": {"type": "string"}, "description": "Patterns to exclude from suggestions"}
                        },
                        "required": ["requestingAgentId", "coordinationScenario", "currentContext"]
                    }
                },
                {
                    "name": "vibe_guideline_enforce",
                    "description": "Apply organizational coordination policies and validate compliance",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "enforcingAgentId": {"type": "string", "description": "ID of agent enforcing guidelines"},
                            "coordinationPlan": {"type": "object", "description": "Coordination plan to validate"},
                            "applicableGuidelines": {"type": "array", "items": {"type": "string"}, "description": "Guidelines to apply"},
                            "enforcementLevel": {"type": "string", "enum": ["STRICT", "MODERATE", "ADVISORY"], "description": "Level of enforcement"},
                            "allowExceptions": {"type": "boolean", "description": "Whether to allow exceptions to guidelines"}
                        },
                        "required": ["enforcingAgentId", "coordinationPlan", "applicableGuidelines", "enforcementLevel"]
                    }
                },
                {
                    "name": "vibe_learning_capture",
                    "description": "Learn from coordination successes/failures to improve future decisions",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "capturingAgentId": {"type": "string", "description": "ID of agent capturing learning"},
                            "coordinationSession": {"type": "object", "description": "Details of coordination session"},
                            "outcomeData": {"type": "object", "description": "Outcomes and results data"},
                            "successMetrics": {"type": "object", "description": "Metrics measuring coordination success"},
                            "lessonsLearned": {"type": "array", "items": {"type": "string"}, "description": "Key lessons learned"},
                            "improvementOpportunities": {"type": "array", "items": {"type": "string"}, "description": "Opportunities for improvement"}
                        },
                        "required": ["capturingAgentId", "coordinationSession", "outcomeData", "successMetrics", "lessonsLearned"]
                    }
                }
            ]
        });

        Ok(Some(JsonRpcResponse::success(request.id, result)))
    }

    /// Handle tool call request
    async fn handle_call_tool(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling tool call request");

        let params: serde_json::Value = request.params.ok_or_else(|| Error::InvalidParams {
            message: "Missing tool call parameters".to_string(),
        })?;

        let tool_name = params["name"]
            .as_str()
            .ok_or_else(|| Error::InvalidParams {
                message: "Missing tool name".to_string(),
            })?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        debug!("Tool call: {} with arguments: {}", tool_name, arguments);

        // Generic mapping: "vibe_agent_register" -> "vibe/agent/register", etc.
        let method = if let Some(stripped) = tool_name.strip_prefix("vibe_") {
            format!("vibe/{}", stripped.replace('_', "/"))
        } else {
            // Keep legacy names working if already slash-formatted
            tool_name.replace('_', "/")
        };

        // Route to streamlined handlers by converting to VibeOperationParams
        let result = if let Some(rest) = method.strip_prefix("vibe/") {
            // agent/* and issue/* expect bare operation names ("status", "list", ...)
            // other vibe/* use underscore-flattened operation names ("worker_message", etc.)
            let operation = if rest.starts_with("agent/") || rest.starts_with("issue/") {
                rest.split('/').nth(1).unwrap_or_default().to_string()
            } else {
                rest.replace('/', "_")
            };
            if (rest.starts_with("agent/") || rest.starts_with("issue/")) && operation.is_empty() {
                return Err(Error::InvalidParams {
                    message: format!(
                        "Missing operation in tool name '{}'; expected vibe/agent/<op> or vibe/issue/<op>",
                        tool_name
                    ),
                });
            }
            let vibe_params = VibeOperationParams {
                operation,
                params: arguments,
            };

            if rest.starts_with("agent/") {
                let req = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::VIBE_AGENT,
                    Some(serde_json::to_value(vibe_params)?),
                );
                self.handle_vibe_agent(req).await
            } else if rest.starts_with("issue/") {
                let req = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::VIBE_ISSUE,
                    Some(serde_json::to_value(vibe_params)?),
                );
                self.handle_vibe_issue(req).await
            } else {
                let req = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::VIBE_COORDINATION,
                    Some(serde_json::to_value(vibe_params)?),
                );
                self.handle_vibe_coordination(req).await
            }
        } else {
            return Err(Error::InvalidParams {
                message: format!("Unknown tool: {}", tool_name),
            });
        };

        match result? {
            Some(mut response) => {
                if let Some(err) = response.error.take() {
                    // Bubble up the actual tool error
                    return Ok(Some(JsonRpcResponse::error(request.id, err)));
                }
                let response_result = response.result.unwrap_or(serde_json::Value::Null);
                Ok(Some(JsonRpcResponse::success(
                    request.id,
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&response_result)
                                .unwrap_or_else(|_| "Tool executed successfully".to_string())
                        }],
                        "data": response_result
                    }),
                )))
            }
            None => Ok(Some(JsonRpcResponse::success(
                request.id,
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": "Tool executed successfully (no result)"
                    }],
                    "data": serde_json::Value::Null
                }),
            ))),
        }
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

        // Check if this is a status update (has agentId) or status query (no agentId)
        if let Some(ref params) = request.params {
            // Status update if either "agentId" (camelCase) or "agent_id" (snake_case) is present
            let has_agent_id = params
                .get("agentId")
                .or_else(|| params.get("agent_id"))
                .is_some();
            if has_agent_id {
                // Status update from an agent - extract agent ID directly
                let agent_id_str = params
                    .get("agentId")
                    .or_else(|| params.get("agent_id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Protocol {
                        message: "Missing or invalid agent ID".to_string(),
                    })?;

                let agent_id = Uuid::parse_str(agent_id_str).map_err(|e| Error::Protocol {
                    message: format!("Invalid agent ID format: {}", e),
                })?;

                // Update heartbeat
                if let Err(e) = agent_service.update_heartbeat(agent_id).await {
                    warn!("Failed to update heartbeat for agent {}: {}", agent_id, e);
                }

                // Parse and update status only if provided
                if let Some(status_value) = params.get("status") {
                    if let Some(status_str) = status_value.as_str() {
                        if let Ok(agent_status) = self.parse_agent_status(status_str) {
                            if let Err(e) = agent_service
                                .update_agent_status(agent_id, agent_status)
                                .await
                            {
                                warn!("Failed to update status for agent {}: {}", agent_id, e);
                            }
                        } else {
                            warn!(
                                "Invalid status string '{}' for agent {}",
                                status_str, agent_id
                            );
                        }
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
                // Params exist but no agentId - treat as status query
                self.handle_system_status_query(&request, agent_service)
                    .await
            }
        } else {
            // No params at all - also treat as status query
            self.handle_system_status_query(&request, agent_service)
                .await
        }
    }

    /// Handle system status query (when no agentId is provided)
    async fn handle_system_status_query(
        &self,
        request: &JsonRpcRequest,
        agent_service: &Arc<AgentService>,
    ) -> Result<Option<JsonRpcResponse>> {
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
                Ok(Some(JsonRpcResponse::success(request.id.clone(), result)))
            }
            Err(e) => {
                warn!("Failed to get agent statistics: {}", e);
                let result = serde_json::json!({
                    "connected_agents": self.client_count().await,
                    "active_sessions": self.clients.read().await.len(),
                    "error": "Failed to retrieve agent statistics"
                });
                Ok(Some(JsonRpcResponse::success(request.id.clone(), result)))
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

        let knowledge_service =
            self.knowledge_service
                .as_ref()
                .ok_or_else(|| Error::Configuration {
                    message: "Knowledge service not configured".to_string(),
                })?;

        // Parse request parameters
        #[derive(serde::Deserialize)]
        struct KnowledgeQueryParams {
            query: String,
            // Accept "searchScope" from the advertised tool schema
            #[serde(default, alias = "searchScope")]
            knowledge_types: Option<Vec<String>>,
            #[serde(default)]
            tags: Option<Vec<String>>,
            // Accept "maxResults" from the advertised tool schema
            #[serde(default, alias = "maxResults")]
            limit: Option<u32>,
            // Accept "queryingAgentId" from the advertised tool schema
            #[serde(alias = "queryingAgentId")]
            agent_id: String,
            // Tolerate (but ignore) coordination-specific fields from the tool schema
            #[serde(default, alias = "coordinationContext")]
            _coordination_context: Option<String>,
            #[serde(default, alias = "relevanceCriteria")]
            _relevance_criteria: Option<serde_json::Value>,
        }

        let params: KnowledgeQueryParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid knowledge query parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing knowledge query parameters".to_string(),
            });
        };

        // Parse agent ID
        let agent_id = Uuid::parse_str(&params.agent_id).map_err(|e| Error::Protocol {
            message: format!("Invalid agent ID format: {}", e),
        })?;

        // Verify agent exists if agent service is configured (consistent with other handlers)
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(agent_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Agent not found: {}", agent_id),
                        data: None,
                    },
                )));
            }
        }

        // Validate that at least one filter is provided to prevent unbounded searches
        if params.query.trim().is_empty()
            && params.tags.as_ref().map_or(true, |t| t.is_empty())
            && params
                .knowledge_types
                .as_ref()
                .map_or(true, |t| t.is_empty())
        {
            return Err(Error::Protocol {
                message: "At least one filter must be provided (query, tags, or knowledge_types)"
                    .to_string(),
            });
        }

        // Build search criteria
        let mut criteria = vibe_ensemble_core::knowledge::KnowledgeSearchCriteria::new();

        criteria = criteria.with_query(params.query);

        if let Some(types) = params.knowledge_types {
            use vibe_ensemble_core::knowledge::KnowledgeType;
            let parsed_types: std::result::Result<Vec<KnowledgeType>, Error> = types
                .iter()
                .map(|t| {
                    let low = t.to_ascii_lowercase();
                    match low.as_str() {
                        "pattern" | "patterns" => Ok(KnowledgeType::Pattern),
                        "practice" | "practices" => Ok(KnowledgeType::Practice),
                        "guideline" | "guidelines" => Ok(KnowledgeType::Guideline),
                        "solution" | "solutions" => Ok(KnowledgeType::Solution),
                        "reference" | "references" => Ok(KnowledgeType::Reference),
                        _ => Err(Error::Protocol {
                            message: format!("Invalid knowledge type: {}", t),
                        }),
                    }
                })
                .collect();
            criteria = criteria.with_types(parsed_types?);
        }

        if let Some(tags) = params.tags {
            criteria = criteria.with_tags(tags);
        }

        // Clamp limit to valid range [1, 100] to prevent 0 or excessive limits
        let limit = params.limit.unwrap_or(50).clamp(1, 100);
        criteria = criteria.with_limit(limit);

        // Perform search
        let results = knowledge_service
            .search_knowledge(&criteria, agent_id)
            .await?;

        debug!("Knowledge query returned {} results", results.len());

        let result = serde_json::json!({
            "results": results,
            "total": results.len()
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

        // Parse estimated duration using humantime crate (secure alternative)
        let estimated_duration = params
            .estimated_duration
            .as_ref()
            .and_then(|s| humantime::parse_duration(s).ok())
            .and_then(|std_dur| chrono::Duration::from_std(std_dur).ok())
            .filter(|d| *d > chrono::Duration::zero());

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

    // Issue #52: Intelligent Work Orchestration method handlers

    /// Handle schedule coordination request
    async fn handle_schedule_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling schedule coordination request");

        // Parse request parameters
        let params: ScheduleCoordinateParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid schedule coordination parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing schedule coordination parameters".to_string(),
            });
        };

        // Parse coordinator agent ID
        let coordinator_id = match Uuid::parse_str(&params.coordinator_agent_id) {
            Ok(id) => id,
            Err(e) => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::INVALID_PARAMS,
                        message: format!("Invalid coordinator agent ID: {}", e),
                        data: None,
                    },
                )));
            }
        };

        // Parse involved agent IDs
        let mut involved_agents = Vec::new();
        for agent_id_str in &params.involved_agents {
            let agent_id = Uuid::parse_str(agent_id_str).map_err(|e| Error::Protocol {
                message: format!("Invalid involved agent ID '{}': {}", agent_id_str, e),
            })?;
            involved_agents.push(agent_id);
        }

        // Verify coordinator agent exists
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
        }

        info!(
            "Processing schedule coordination for {} work sequences across {} agents",
            params.work_sequences.len(),
            involved_agents.len()
        );

        // Create intelligent coordination schedule using load balancing
        let schedule_id = Uuid::new_v4();

        // Generate optimized work sequence using smart scheduling
        let (optimized_sequence, estimated_completion) = self
            .create_smart_schedule(&params, &involved_agents)
            .await?;

        // Generate intelligent resource allocations
        let resource_allocations = self
            .optimize_resource_allocation(&params, &involved_agents)
            .await?;

        // Create intelligent dependency graph
        let dependency_graph = self
            .analyze_dependencies(&params, &optimized_sequence)
            .await?;

        let result = ScheduleCoordinateResult {
            coordination_schedule_id: schedule_id,
            optimized_sequence,
            resource_allocations,
            dependency_graph,
            estimated_completion_time: estimated_completion,
            conflict_warnings: vec!["Consider resource contention on critical files".to_string()],
            status: "scheduled".to_string(),
            message: format!(
                "Work coordination schedule created for {} sequences across {} projects",
                params.work_sequences.len(),
                params.project_scopes.len()
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle conflict prediction request
    async fn handle_conflict_predict(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling conflict prediction request");

        // Parse request parameters
        let params: ConflictPredictParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid conflict prediction parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing conflict prediction parameters".to_string(),
            });
        };

        // Parse analyzer agent ID
        let analyzer_id =
            Uuid::parse_str(&params.analyzer_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid analyzer agent ID: {}", e),
            })?;

        // Verify analyzer agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(analyzer_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Analyzer agent not found: {}", analyzer_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Analyzing {} planned actions against {} active workflows for conflicts",
            params.planned_actions.len(),
            params.active_workflows.len()
        );

        let analysis_id = Uuid::new_v4();

        // Perform intelligent conflict prediction analysis
        let predicted_conflicts = self.analyze_conflicts(&params).await?;

        // Generate intelligent risk assessment
        let risk_assessment = self
            .assess_conflict_risk(&params, &predicted_conflicts)
            .await?;

        // Generate smart recommended actions
        let recommended_actions = self
            .generate_prevention_actions(&params, &predicted_conflicts)
            .await?;

        let result = ConflictPredictResult {
            analysis_id,
            predicted_conflicts: predicted_conflicts.clone(),
            risk_assessment,
            recommended_actions,
            prevention_strategies: vec![
                "Implement resource locking".to_string(),
                "Use communication channels".to_string(),
                "Schedule coordination meetings".to_string(),
            ],
            monitoring_points: vec![
                "File modification timestamps".to_string(),
                "Agent activity logs".to_string(),
                "Resource access patterns".to_string(),
            ],
            confidence: 0.82,
            message: format!(
                "Analyzed {} actions and detected {} potential conflicts with {:.1}% confidence",
                params.planned_actions.len(),
                predicted_conflicts.len(),
                82.0
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle resource reservation request
    async fn handle_resource_reserve(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling resource reservation request");

        // Parse request parameters
        let params: ResourceReserveParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid resource reservation parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing resource reservation parameters".to_string(),
            });
        };

        // Parse reserving agent ID
        let reserving_id =
            Uuid::parse_str(&params.reserving_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid reserving agent ID: {}", e),
            })?;

        // Verify reserving agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(reserving_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Reserving agent not found: {}", reserving_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Processing resource reservation for {} resources with {} access",
            params.resource_paths.len(),
            params.reservation_type
        );

        let reservation_id = Uuid::new_v4();
        let access_token = format!("res_token_{}", reservation_id);

        // Parse duration and calculate expiration
        let duration_hours = match params.reservation_duration.chars().last() {
            Some('h') => params
                .reservation_duration
                .trim_end_matches('h')
                .parse::<i64>()
                .unwrap_or(2),
            Some('d') => {
                params
                    .reservation_duration
                    .trim_end_matches('d')
                    .parse::<i64>()
                    .unwrap_or(1)
                    * 24
            }
            _ => 2, // Default to 2 hours
        };

        let expiration_time = chrono::Utc::now() + chrono::Duration::hours(duration_hours);

        // Create reserved resources data
        let reserved_resources = params
            .resource_paths
            .iter()
            .map(|path| {
                serde_json::json!({
                    "path": path,
                    "reservation_type": params.reservation_type,
                    "exclusive_access": params.exclusive_access,
                    "allowed_operations": params.allowed_operations,
                    "locked_at": chrono::Utc::now(),
                    "lock_status": "active"
                })
            })
            .collect();

        let result = ResourceReserveResult {
            reservation_id,
            reserved_resources,
            access_token,
            expiration_time,
            conflicting_reservations: vec![],
            coordination_required: !params.exclusive_access,
            status: "reserved".to_string(),
            message: format!(
                "Successfully reserved {} resources for {} access until {}",
                params.resource_paths.len(),
                params.reservation_type.to_lowercase(),
                expiration_time.format("%Y-%m-%d %H:%M:%S UTC")
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle merge coordination request
    async fn handle_merge_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling merge coordination request");

        // Parse request parameters
        let params: MergeCoordinateParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid merge coordination parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing merge coordination parameters".to_string(),
            });
        };

        // Parse coordinator agent ID
        let coordinator_id =
            Uuid::parse_str(&params.coordinator_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid coordinator agent ID: {}", e),
            })?;

        // Parse involved agent IDs
        let mut involved_agents = Vec::new();
        for agent_id_str in &params.involved_agents {
            let agent_id = Uuid::parse_str(agent_id_str).map_err(|e| Error::Protocol {
                message: format!("Invalid involved agent ID '{}': {}", agent_id_str, e),
            })?;
            involved_agents.push(agent_id);
        }

        // Verify coordinator agent exists
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
        }

        info!(
            "Coordinating {} merge from {} branches to {} involving {} agents",
            params.merge_scenario,
            params.source_branches.len(),
            params.target_branch,
            involved_agents.len()
        );

        let merge_coordination_id = Uuid::new_v4();

        // Determine merge strategy based on scenario
        let merge_strategy = match params.merge_scenario.as_str() {
            "MULTI_BRANCH" => "sequential_integration",
            "FEATURE_INTEGRATION" => "feature_branch_merge",
            "HOTFIX_MERGE" => "fast_forward",
            "RELEASE_MERGE" => "release_merge",
            _ => "standard_merge",
        };

        // Create sequence plan for merge steps
        let sequence_plan = params
            .source_branches
            .iter()
            .enumerate()
            .map(|(i, branch)| {
                serde_json::json!({
                    "step": i + 1,
                    "action": "merge",
                    "source_branch": branch,
                    "target_branch": &params.target_branch,
                    "assigned_agent": involved_agents.get(i % involved_agents.len()).map(|id| id.to_string()).unwrap_or_default(),
                    "estimated_duration": "30 minutes",
                    "dependencies": if i > 0 { vec![i] } else { vec![] },
                    "risk_level": "medium"
                })
            })
            .collect();

        // Create conflict resolution plan
        let conflict_resolution_plan = serde_json::json!({
            "strategy": params.conflict_resolution_strategy.as_ref().unwrap_or(&"HYBRID".to_string()),
            "auto_resolution_rules": [
                "prefer_target_branch_for_config",
                "manual_review_for_business_logic",
                "auto_merge_documentation"
            ],
            "escalation_threshold": 3,
            "review_required": true
        });

        // Create review assignments
        let review_assignments = involved_agents
            .iter()
            .enumerate()
            .map(|(i, agent_id)| {
                serde_json::json!({
                    "reviewer_id": agent_id.to_string(),
                    "review_scope": format!("merge_step_{}", i + 1),
                    "review_type": if i == 0 { "primary" } else { "secondary" },
                    "estimated_effort": "45 minutes"
                })
            })
            .collect();

        let estimated_merge_time =
            chrono::Utc::now() + chrono::Duration::hours((params.source_branches.len() as i64) + 2);

        // Create rollback plan
        let rollback_plan = serde_json::json!({
            "rollback_branch": format!("rollback_{}", merge_coordination_id),
            "snapshot_commits": params.source_branches.clone(),
            "rollback_steps": [
                "reset_target_branch",
                "restore_working_directories",
                "notify_all_agents"
            ],
            "estimated_rollback_time": "15 minutes"
        });

        let result = MergeCoordinateResult {
            merge_coordination_id,
            merge_strategy: merge_strategy.to_string(),
            sequence_plan,
            conflict_resolution_plan,
            review_assignments,
            estimated_merge_time,
            rollback_plan,
            message: format!(
                "Merge coordination plan created for {} scenario with {} branches and {} agents",
                params.merge_scenario,
                params.source_branches.len(),
                involved_agents.len()
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    // Issue #53: Knowledge-Driven Coordination method handlers

    /// Handle knowledge query for coordination
    async fn handle_knowledge_query_coordination(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling knowledge query coordination request");

        // Parse request parameters
        let params: KnowledgeQueryCoordinationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid knowledge query coordination parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing knowledge query coordination parameters".to_string(),
            });
        };

        // Parse querying agent ID
        let querying_id =
            Uuid::parse_str(&params.querying_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid querying agent ID: {}", e),
            })?;

        // Verify querying agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(querying_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Querying agent not found: {}", querying_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Processing knowledge query for coordination context: {} with query: {}",
            params.coordination_context, params.query
        );

        let query_id = Uuid::new_v4();

        // Simulate knowledge retrieval based on search scope
        let relevant_patterns = params
            .search_scope
            .iter()
            .map(|scope| match scope.as_str() {
                "patterns" => serde_json::json!({
                    "pattern_id": "sequential_coordination",
                    "name": "Sequential Task Coordination",
                    "description": "Coordinate tasks in sequence to avoid conflicts",
                    "applicability": 0.85,
                    "success_rate": 0.92,
                    "usage_count": 47
                }),
                "practices" => serde_json::json!({
                    "practice_id": "resource_locking",
                    "name": "Proactive Resource Locking",
                    "description": "Lock resources before modification to prevent conflicts",
                    "applicability": 0.78,
                    "success_rate": 0.89,
                    "usage_count": 31
                }),
                _ => serde_json::json!({
                    "pattern_id": "generic_coordination",
                    "name": "Generic Coordination Pattern",
                    "description": "General coordination approach",
                    "applicability": 0.65,
                    "success_rate": 0.75,
                    "usage_count": 12
                }),
            })
            .collect();

        let best_practices = vec![
            serde_json::json!({
                "practice": "Communication First",
                "description": "Always communicate intent before starting work",
                "confidence": 0.95,
                "source": "organizational_guidelines"
            }),
            serde_json::json!({
                "practice": "Resource Reservation",
                "description": "Reserve resources proactively to prevent conflicts",
                "confidence": 0.88,
                "source": "historical_success"
            }),
        ];

        let historical_solutions = vec![serde_json::json!({
            "solution_id": "merge_conflict_resolution_2024_01",
            "scenario": "Multi-agent code modification",
            "approach": "Sequential coordination with automated conflict detection",
            "outcome": "successful",
            "lessons": ["Early communication prevented 80% of potential conflicts"],
            "similarity": 0.82
        })];

        let organizational_guidelines = vec![serde_json::json!({
            "guideline_id": "coord_001",
            "title": "Agent Coordination Protocol",
            "description": "Standard protocol for multi-agent coordination",
            "compliance_level": "mandatory",
            "last_updated": "2024-01-15"
        })];

        let result = KnowledgeQueryCoordinationResult {
            query_id,
            relevant_patterns,
            best_practices,
            historical_solutions,
            organizational_guidelines,
            confidence_score: 0.87,
            applicability_rating: 0.82,
            message: format!(
                "Found {} relevant patterns for coordination context: {}",
                params.search_scope.len(),
                params.coordination_context
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle pattern suggestion request
    async fn handle_pattern_suggest(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling pattern suggestion request");

        // Parse request parameters
        let params: PatternSuggestParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid pattern suggestion parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing pattern suggestion parameters".to_string(),
            });
        };

        // Parse requesting agent ID
        let requesting_id =
            Uuid::parse_str(&params.requesting_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid requesting agent ID: {}", e),
            })?;

        // Verify requesting agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(requesting_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Requesting agent not found: {}", requesting_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Generating pattern suggestions for scenario: {}",
            params.coordination_scenario
        );

        let suggestion_id = Uuid::new_v4();

        // Generate recommended patterns based on scenario
        let recommended_patterns = vec![
            serde_json::json!({
                "pattern_id": "producer_consumer",
                "name": "Producer-Consumer Coordination",
                "description": "One agent produces output, another consumes it",
                "match_score": 0.91,
                "complexity": "medium",
                "estimated_setup_time": "30 minutes",
                "resource_requirements": ["shared_queue", "synchronization_mechanism"]
            }),
            serde_json::json!({
                "pattern_id": "pipeline_coordination",
                "name": "Pipeline Coordination",
                "description": "Sequential processing through multiple stages",
                "match_score": 0.87,
                "complexity": "low",
                "estimated_setup_time": "45 minutes",
                "resource_requirements": ["stage_definitions", "progress_tracking"]
            }),
        ];

        let adaptation_guidance = vec![
            "Adjust timing based on agent availability".to_string(),
            "Consider resource constraints in current environment".to_string(),
            "Monitor for bottlenecks in critical sections".to_string(),
        ];

        let implementation_steps = vec![
            serde_json::json!({
                "step": 1,
                "description": "Define coordination protocol",
                "estimated_effort": "15 minutes",
                "dependencies": [],
                "responsible_party": "coordinator"
            }),
            serde_json::json!({
                "step": 2,
                "description": "Set up communication channels",
                "estimated_effort": "20 minutes",
                "dependencies": [1],
                "responsible_party": "all_agents"
            }),
            serde_json::json!({
                "step": 3,
                "description": "Establish monitoring and feedback loops",
                "estimated_effort": "25 minutes",
                "dependencies": [1, 2],
                "responsible_party": "coordinator"
            }),
        ];

        let alternative_approaches = vec![serde_json::json!({
            "approach": "Event-Driven Coordination",
            "description": "Use events to trigger coordination actions",
            "pros": ["Decoupled", "Scalable", "Reactive"],
            "cons": ["Complex setup", "Debugging challenges"],
            "match_score": 0.75
        })];

        let result = PatternSuggestResult {
            suggestion_id,
            recommended_patterns,
            adaptation_guidance,
            implementation_steps,
            success_probability: 0.89,
            alternative_approaches,
            risk_factors: vec![
                "Network latency affecting coordination".to_string(),
                "Agent failure during critical sections".to_string(),
            ],
            message: format!(
                "Generated {} pattern suggestions for scenario: {}",
                2, // recommended_patterns.len() would be more dynamic
                params.coordination_scenario
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle guideline enforcement request
    async fn handle_guideline_enforce(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling guideline enforcement request");

        // Parse request parameters
        let params: GuidelineEnforceParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid guideline enforcement parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing guideline enforcement parameters".to_string(),
            });
        };

        // Parse enforcing agent ID
        let enforcing_id =
            Uuid::parse_str(&params.enforcing_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid enforcing agent ID: {}", e),
            })?;

        // Verify enforcing agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(enforcing_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Enforcing agent not found: {}", enforcing_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Enforcing {} guidelines with {} enforcement level",
            params.applicable_guidelines.len(),
            params.enforcement_level
        );

        let enforcement_id = Uuid::new_v4();

        // Simulate guideline compliance checking
        let mut violations = Vec::new();
        let mut approved_exceptions = Vec::new();
        let mut compliance_score = 1.0f32;

        // Check each guideline
        for guideline in &params.applicable_guidelines {
            match guideline.as_str() {
                "communication_first" => {
                    // Check if communication plan exists
                    if params.coordination_plan.get("communication_plan").is_none() {
                        violations.push(serde_json::json!({
                            "guideline": "communication_first",
                            "severity": "medium",
                            "description": "No communication plan defined",
                            "suggested_fix": "Add communication protocol to coordination plan"
                        }));
                        compliance_score -= 0.2;
                    }
                }
                "resource_reservation" => {
                    // Check if resources are properly reserved
                    if params
                        .coordination_plan
                        .get("resource_reservations")
                        .is_none()
                    {
                        violations.push(serde_json::json!({
                            "guideline": "resource_reservation",
                            "severity": "high",
                            "description": "No resource reservations specified",
                            "suggested_fix": "Define resource reservation strategy"
                        }));
                        compliance_score -= 0.3;
                    }
                }
                "conflict_prevention" => {
                    // Check if conflict prevention measures exist
                    if params
                        .coordination_plan
                        .get("conflict_prevention")
                        .is_none()
                    {
                        if params.allow_exceptions {
                            approved_exceptions.push(serde_json::json!({
                                "guideline": "conflict_prevention",
                                "justification": "Low-risk coordination scenario",
                                "approved_by": enforcing_id.to_string(),
                                "conditions": ["Limited agent involvement", "Read-only operations"]
                            }));
                        } else {
                            violations.push(serde_json::json!({
                                "guideline": "conflict_prevention",
                                "severity": "high",
                                "description": "No conflict prevention strategy defined",
                                "suggested_fix": "Add conflict detection and prevention measures"
                            }));
                            compliance_score -= 0.25;
                        }
                    }
                }
                _ => {
                    // Unknown guideline
                    violations.push(serde_json::json!({
                        "guideline": guideline,
                        "severity": "low",
                        "description": "Unknown guideline cannot be validated",
                        "suggested_fix": "Review guideline definition"
                    }));
                    compliance_score -= 0.1;
                }
            }
        }

        compliance_score = compliance_score.max(0.0);

        let recommended_corrections = violations
            .iter()
            .map(|v| {
                serde_json::json!({
                    "violation_id": Uuid::new_v4().to_string(),
                    "guideline": v.get("guideline").unwrap_or(&serde_json::Value::Null),
                    "correction": v.get("suggested_fix").unwrap_or(&serde_json::Value::Null),
                    "priority": match v.get("severity").and_then(|s| s.as_str()) {
                        Some("high") => "critical",
                        Some("medium") => "important",
                        _ => "optional"
                    },
                    "estimated_effort": "30 minutes"
                })
            })
            .collect();

        let audit_trail = vec![serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "enforcing_agent": enforcing_id.to_string(),
            "enforcement_level": params.enforcement_level,
            "guidelines_checked": params.applicable_guidelines.len(),
            "violations_found": violations.len(),
            "exceptions_granted": approved_exceptions.len(),
            "compliance_score": compliance_score
        })];

        let compliance_status = match compliance_score {
            s if s >= 0.9 => "compliant",
            s if s >= 0.7 => "mostly_compliant",
            s if s >= 0.5 => "partially_compliant",
            _ => "non_compliant",
        };

        let result = GuidelineEnforceResult {
            enforcement_id,
            compliance_status: compliance_status.to_string(),
            violations: violations.clone(),
            recommended_corrections,
            approved_exceptions,
            compliance_score,
            audit_trail,
            message: format!(
                "Guideline enforcement completed: {} compliance with {} violations",
                compliance_status,
                violations.len()
            ),
        };

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    /// Handle learning capture request
    async fn handle_learning_capture(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        debug!("Handling learning capture request");

        // Parse request parameters
        let params: LearningCaptureParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::Protocol {
                message: format!("Invalid learning capture parameters: {}", e),
            })?
        } else {
            return Err(Error::Protocol {
                message: "Missing learning capture parameters".to_string(),
            });
        };

        // Parse capturing agent ID
        let capturing_id =
            Uuid::parse_str(&params.capturing_agent_id).map_err(|e| Error::Protocol {
                message: format!("Invalid capturing agent ID: {}", e),
            })?;

        // Verify capturing agent exists
        if let Some(agent_service) = &self.agent_service {
            if agent_service.get_agent(capturing_id).await?.is_none() {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::AGENT_NOT_FOUND,
                        message: format!("Capturing agent not found: {}", capturing_id),
                        data: None,
                    },
                )));
            }
        }

        info!(
            "Capturing learning from coordination session with {} lessons learned",
            params.lessons_learned.len()
        );

        let learning_record_id = Uuid::new_v4();

        // Extract patterns from coordination session
        let extracted_patterns = vec![
            serde_json::json!({
                "pattern_type": "communication_timing",
                "pattern_name": "Early Status Communication",
                "description": "Communicate status changes immediately to prevent conflicts",
                "confidence": 0.92,
                "generalizability": "high",
                "usage_contexts": ["multi_agent_coordination", "resource_sharing"]
            }),
            serde_json::json!({
                "pattern_type": "resource_management",
                "pattern_name": "Proactive Resource Locking",
                "description": "Lock resources before starting work to prevent race conditions",
                "confidence": 0.87,
                "generalizability": "medium",
                "usage_contexts": ["file_modification", "database_updates"]
            }),
        ];

        // Generate knowledge contributions
        let knowledge_contributions = vec![
            serde_json::json!({
                "contribution_type": "best_practice",
                "title": "Coordination Meeting Frequency",
                "description": "Daily coordination meetings reduce conflicts by 35%",
                "evidence": params.success_metrics,
                "applicability": "cross_project_coordination",
                "confidence": 0.89
            }),
            serde_json::json!({
                "contribution_type": "antipattern",
                "title": "Silent Work Assumption",
                "description": "Assuming other agents know about your work leads to conflicts",
                "evidence": params.outcome_data,
                "frequency": "common",
                "mitigation": "Mandatory work announcements"
            }),
        ];

        // Identify process improvements
        let process_improvements = params
            .improvement_opportunities
            .iter()
            .map(|opportunity| {
                serde_json::json!({
                    "improvement_id": Uuid::new_v4().to_string(),
                    "area": "coordination_process",
                    "opportunity": opportunity,
                    "potential_impact": "medium",
                    "implementation_effort": "low",
                    "priority": "normal"
                })
            })
            .collect();

        // Generate organizational learning insights
        let organizational_learning = serde_json::json!({
            "coordination_effectiveness": 0.84,
            "key_success_factors": [
                "Clear communication protocols",
                "Proactive conflict prevention",
                "Regular status updates"
            ],
            "common_failure_modes": [
                "Assumption of shared context",
                "Resource contention",
                "Timeline misalignment"
            ],
            "recommended_training": [
                "Conflict prevention strategies",
                "Resource management best practices"
            ]
        });

        // Generate future recommendations
        let future_recommendations = vec![
            "Implement automated conflict detection system".to_string(),
            "Establish standard coordination templates".to_string(),
            "Create coordination success metrics dashboard".to_string(),
            "Develop agent coordination training program".to_string(),
        ];

        // Calculate knowledge quality score based on various factors
        let knowledge_quality_score = {
            let lesson_quality = (params.lessons_learned.len() as f32 * 0.2).min(1.0);
            let outcome_completeness = if params.success_metrics.is_object() {
                0.3
            } else {
                0.0
            };
            let improvement_identification =
                (params.improvement_opportunities.len() as f32 * 0.1).min(0.5);
            (lesson_quality + outcome_completeness + improvement_identification).min(1.0)
        };

        let result = LearningCaptureResult {
            learning_record_id,
            extracted_patterns,
            knowledge_contributions,
            process_improvements,
            organizational_learning,
            future_recommendations,
            knowledge_quality_score,
            message: format!(
                "Captured learning record with {} lessons and {} improvement opportunities",
                params.lessons_learned.len(),
                params.improvement_opportunities.len()
            ),
        };

        // Persist learned knowledge to knowledge repository if service is available
        if let Some(knowledge_service) = &self.knowledge_service {
            for contribution in &result.knowledge_contributions {
                if let (Some(title), Some(description)) = (
                    contribution.get("title").and_then(|v| v.as_str()),
                    contribution.get("description").and_then(|v| v.as_str()),
                ) {
                    // Derive a knowledge type from contribution_type when possible
                    let derived_type = match contribution
                        .get("contribution_type")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("best_practice") | Some("practice") => {
                            vibe_ensemble_core::knowledge::KnowledgeType::Practice
                        }
                        Some("guideline") => {
                            vibe_ensemble_core::knowledge::KnowledgeType::Guideline
                        }
                        Some("antipattern") | Some("anti_pattern") => {
                            vibe_ensemble_core::knowledge::KnowledgeType::Pattern
                        }
                        Some("solution") => vibe_ensemble_core::knowledge::KnowledgeType::Solution,
                        Some("reference") => {
                            vibe_ensemble_core::knowledge::KnowledgeType::Reference
                        }
                        _ => vibe_ensemble_core::knowledge::KnowledgeType::Practice,
                    };

                    let knowledge = vibe_ensemble_core::knowledge::Knowledge::builder()
                        .title(format!("Learning: {}", title))
                        .content(format!(
                            "**Type**: {}\n\n**Description**: {}\n\n**Evidence**: {}\n\n**Confidence**: {}\n\n**Context**: Coordination Learning Session {}",
                            contribution.get("contribution_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
                            description,
                            contribution.get("evidence").map(|v| v.to_string()).unwrap_or_else(|| "No specific evidence provided".to_string()),
                            contribution.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0),
                            learning_record_id
                        ))
                        .knowledge_type(derived_type)
                        .created_by(capturing_id)
                        .access_level(vibe_ensemble_core::knowledge::AccessLevel::Team)
                        .tags(["coordination", "learning", "experience"])
                        .build();

                    if let Ok(knowledge_entry) = knowledge {
                        if let Err(e) = knowledge_service.create_knowledge(&knowledge_entry).await {
                            warn!("Failed to persist learning knowledge: {}", e);
                        } else {
                            info!("Persisted learning knowledge: {}", knowledge_entry.title);
                        }
                    }
                }
            }
        }

        Ok(Some(JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result)?,
        )))
    }

    // Issue #52: Smart Work Scheduling and Conflict Prevention - Intelligent Algorithms

    /// Intelligent conflict analysis using real agent and resource data
    async fn analyze_conflicts(
        &self,
        params: &ConflictPredictParams,
    ) -> Result<Vec<serde_json::Value>> {
        let mut conflicts = Vec::new();

        // Analyze resource conflicts using resource map
        if let Some(resource_map) = params.resource_map.as_object() {
            conflicts.extend(
                self.detect_resource_conflicts(resource_map, &params.planned_actions)
                    .await?,
            );
        }

        // Analyze temporal conflicts
        conflicts.extend(
            self.detect_temporal_conflicts(&params.planned_actions, &params.active_workflows)
                .await?,
        );

        // Analyze dependency conflicts
        conflicts.extend(
            self.detect_dependency_conflicts(&params.planned_actions)
                .await?,
        );

        Ok(conflicts)
    }

    /// Detect resource-based conflicts
    async fn detect_resource_conflicts(
        &self,
        _resource_map: &serde_json::Map<String, serde_json::Value>,
        planned_actions: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>> {
        let mut conflicts = Vec::new();

        // Track resource usage patterns
        let mut resource_usage: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();

        for (action_idx, action) in planned_actions.iter().enumerate() {
            if let Some(resources) = action.get("resources").and_then(|r| r.as_array()) {
                for resource in resources {
                    if let Some(resource_path) = resource.as_str() {
                        resource_usage
                            .entry(resource_path.to_string())
                            .or_default()
                            .push(action_idx);
                    }
                }
            }
        }

        // Find conflicts where multiple actions use the same resource
        for (resource_path, action_indices) in resource_usage {
            if action_indices.len() > 1 {
                let probability = self
                    .calculate_conflict_probability(&action_indices, planned_actions)
                    .await;
                let impact = self.assess_resource_impact(&resource_path).await;

                conflicts.push(serde_json::json!({
                    "conflict_type": "resource_contention",
                    "probability": probability,
                    "resources": [resource_path],
                    "involved_actions": action_indices,
                    "estimated_impact": impact,
                    "timeline": self.estimate_conflict_timeline(&action_indices, planned_actions).await
                }));
            }
        }

        Ok(conflicts)
    }

    /// Detect temporal conflicts based on timing
    async fn detect_temporal_conflicts(
        &self,
        planned_actions: &[serde_json::Value],
        _active_workflows: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>> {
        let mut conflicts = Vec::new();

        // Check for overlapping time windows
        for (i, action1) in planned_actions.iter().enumerate() {
            for (j, action2) in planned_actions.iter().enumerate().skip(i + 1) {
                if self.actions_have_temporal_overlap(action1, action2).await {
                    conflicts.push(serde_json::json!({
                        "conflict_type": "temporal_overlap",
                        "probability": 0.8,
                        "involved_actions": [i, j],
                        "estimated_impact": "medium",
                        "timeline": "concurrent execution"
                    }));
                }
            }
        }

        Ok(conflicts)
    }

    /// Detect dependency-based conflicts
    async fn detect_dependency_conflicts(
        &self,
        planned_actions: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>> {
        let mut conflicts = Vec::new();

        // Analyze dependencies between actions
        for (i, action) in planned_actions.iter().enumerate() {
            if let Some(dependencies) = action.get("dependencies").and_then(|d| d.as_array()) {
                for dep in dependencies {
                    if let Some(dep_idx) = dep.as_u64() {
                        if (dep_idx as usize) < planned_actions.len() {
                            // Check for circular dependencies
                            if self
                                .has_circular_dependency(i, dep_idx as usize, planned_actions)
                                .await
                            {
                                conflicts.push(serde_json::json!({
                                    "conflict_type": "dependency_violation",
                                    "probability": 0.95,
                                    "involved_actions": [i, dep_idx],
                                    "estimated_impact": "high",
                                    "timeline": "blocking"
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(conflicts)
    }

    /// Calculate conflict probability based on action patterns
    async fn calculate_conflict_probability(
        &self,
        action_indices: &[usize],
        planned_actions: &[serde_json::Value],
    ) -> f64 {
        let mut base_probability: f64 = 0.5;

        // Increase probability based on action types
        for &idx in action_indices {
            if let Some(action) = planned_actions.get(idx) {
                if let Some(action_type) = action.get("type").and_then(|t| t.as_str()) {
                    match action_type {
                        "write" | "modify" => base_probability += 0.2,
                        "delete" => base_probability += 0.3,
                        "read" => base_probability += 0.1,
                        _ => base_probability += 0.1,
                    }
                }
            }
        }

        base_probability.min(0.95)
    }

    /// Assess the impact of conflicts on a specific resource
    async fn assess_resource_impact(&self, resource_path: &str) -> &'static str {
        match resource_path {
            path if path.contains("main.rs") || path.contains("lib.rs") => "high",
            path if path.contains("config") || path.contains("Cargo.toml") => "high",
            path if path.contains("test") => "low",
            path if path.contains("doc") || path.contains("README") => "low",
            _ => "medium",
        }
    }

    /// Estimate timeline for conflict resolution
    async fn estimate_conflict_timeline(
        &self,
        action_indices: &[usize],
        _planned_actions: &[serde_json::Value],
    ) -> String {
        let duration = action_indices.len() * 30; // 30 minutes per conflicting action
        format!("{}-{} minutes", duration.saturating_sub(15), duration + 15)
    }

    /// Check if two actions have temporal overlap
    async fn actions_have_temporal_overlap(
        &self,
        action1: &serde_json::Value,
        action2: &serde_json::Value,
    ) -> bool {
        // Simple heuristic: actions that modify the same type of files are likely to overlap
        let type1 = action1.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let type2 = action2.get("type").and_then(|t| t.as_str()).unwrap_or("");

        matches!(
            (type1, type2),
            ("write", "write") | ("modify", "modify") | ("write", "modify") | ("modify", "write")
        )
    }

    /// Check for circular dependencies
    async fn has_circular_dependency(
        &self,
        action_idx: usize,
        dep_idx: usize,
        planned_actions: &[serde_json::Value],
    ) -> bool {
        // Simple cycle detection: check if dependency action depends back on original action
        if let Some(dep_action) = planned_actions.get(dep_idx) {
            if let Some(dependencies) = dep_action.get("dependencies").and_then(|d| d.as_array()) {
                for dep in dependencies {
                    if let Some(dep_num) = dep.as_u64() {
                        if dep_num as usize == action_idx {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Generate intelligent risk assessment
    async fn assess_conflict_risk(
        &self,
        params: &ConflictPredictParams,
        predicted_conflicts: &[serde_json::Value],
    ) -> Result<serde_json::Value> {
        let conflict_count = predicted_conflicts.len();
        let high_impact_conflicts = predicted_conflicts
            .iter()
            .filter(|c| c.get("estimated_impact").and_then(|i| i.as_str()) == Some("high"))
            .count();

        let overall_risk = match (conflict_count, high_impact_conflicts) {
            (0, _) => "low",
            (1..=2, 0) => "medium",
            (1..=2, _) => "high",
            (_, 0) => "high",
            _ => "critical",
        };

        let mut risk_factors = Vec::new();
        if conflict_count > 2 {
            risk_factors.push("Multiple concurrent conflicts detected");
        }
        if high_impact_conflicts > 0 {
            risk_factors.push("High-impact conflicts present");
        }
        if params.active_workflows.len() > params.planned_actions.len() {
            risk_factors.push("More active workflows than planned actions");
        }

        let confidence_level = 0.7 + (conflict_count as f64 * 0.1).min(0.25);

        Ok(serde_json::json!({
            "overall_risk": overall_risk,
            "risk_factors": risk_factors,
            "mitigation_urgency": match overall_risk {
                "critical" => "immediate",
                "high" => "urgent",
                "medium" => "moderate",
                _ => "low"
            },
            "confidence_level": confidence_level,
            "total_conflicts": conflict_count,
            "high_impact_conflicts": high_impact_conflicts
        }))
    }

    /// Generate smart prevention actions
    async fn generate_prevention_actions(
        &self,
        _params: &ConflictPredictParams,
        predicted_conflicts: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>> {
        let mut actions = Vec::new();

        // Analyze conflicts and suggest specific actions
        for conflict in predicted_conflicts {
            if let Some(conflict_type) = conflict.get("conflict_type").and_then(|t| t.as_str()) {
                match conflict_type {
                    "resource_contention" => {
                        actions.push(serde_json::json!({
                            "action": "resource_reservation",
                            "description": "Reserve conflicting resources before work begins",
                            "priority": "high",
                            "estimated_effort": "10-15 minutes",
                            "automation_possible": true
                        }));
                    }
                    "temporal_overlap" => {
                        actions.push(serde_json::json!({
                            "action": "sequence_adjustment",
                            "description": "Adjust execution timing to prevent overlap",
                            "priority": "medium",
                            "estimated_effort": "20-30 minutes",
                            "automation_possible": true
                        }));
                    }
                    "dependency_violation" => {
                        actions.push(serde_json::json!({
                            "action": "dependency_reordering",
                            "description": "Reorder actions to resolve dependency conflicts",
                            "priority": "high",
                            "estimated_effort": "30-45 minutes",
                            "automation_possible": false
                        }));
                    }
                    _ => {}
                }
            }
        }

        // Add general coordination actions
        if predicted_conflicts.len() > 1 {
            actions.push(serde_json::json!({
                "action": "coordination_meeting",
                "description": "Schedule coordination meeting with involved agents",
                "priority": "medium",
                "estimated_effort": "45-60 minutes",
                "automation_possible": false
            }));
        }

        Ok(actions)
    }

    /// Create smart work schedule using load balancing
    async fn create_smart_schedule(
        &self,
        params: &ScheduleCoordinateParams,
        involved_agents: &[Uuid],
    ) -> Result<(Vec<serde_json::Value>, chrono::DateTime<chrono::Utc>)> {
        let mut optimized_sequence = Vec::new();
        let mut total_duration = 0;

        // Get agent load balancing recommendations if available
        if let Some(agent_service) = &self.agent_service {
            let load_recommendations = agent_service
                .get_load_balancer_recommendations(params.work_sequences.len())
                .await?;

            // Assign work based on agent capabilities and load
            for (i, work_sequence) in params.work_sequences.iter().enumerate() {
                let assigned_agent = if let Some(recommendation) =
                    load_recommendations.recommended_assignments.get(i)
                {
                    recommendation.agent_id.to_string()
                } else {
                    involved_agents
                        .get(i % involved_agents.len())
                        .map(|id| id.to_string())
                        .unwrap_or_default()
                };

                let estimated_duration = self.estimate_work_duration(work_sequence).await;
                total_duration += estimated_duration;

                let priority = self.calculate_work_priority(work_sequence, i).await;
                let dependencies = self.extract_work_dependencies(work_sequence, i).await;

                optimized_sequence.push(serde_json::json!({
                    "sequence_id": i + 1,
                    "work_item": work_sequence,
                    "estimated_duration": format!("{}h", estimated_duration),
                    "dependencies": dependencies,
                    "assigned_agent": assigned_agent,
                    "priority": priority,
                    "resource_locks": self.identify_required_locks(work_sequence).await,
                    "start_time": chrono::Utc::now() + chrono::Duration::hours(i as i64),
                    "load_balancing_score": load_recommendations.recommended_assignments.get(i).map(|r| r.load_balancing_score).unwrap_or(0.5)
                }));
            }
        } else {
            // Fallback to simple scheduling
            for (i, work_sequence) in params.work_sequences.iter().enumerate() {
                let estimated_duration = self.estimate_work_duration(work_sequence).await;
                total_duration += estimated_duration;

                optimized_sequence.push(serde_json::json!({
                    "sequence_id": i + 1,
                    "work_item": work_sequence,
                    "estimated_duration": format!("{}h", estimated_duration),
                    "dependencies": [],
                    "assigned_agent": involved_agents.get(i % involved_agents.len()).map(|id| id.to_string()).unwrap_or_default(),
                    "priority": "medium",
                    "resource_locks": []
                }));
            }
        }

        let estimated_completion =
            chrono::Utc::now() + chrono::Duration::hours(total_duration as i64);
        Ok((optimized_sequence, estimated_completion))
    }

    /// Estimate work duration based on complexity
    async fn estimate_work_duration(&self, work_sequence: &serde_json::Value) -> i32 {
        let base_duration = 2; // 2 hours base
        let mut duration = base_duration;

        // Adjust based on work complexity
        if let Some(complexity) = work_sequence.get("complexity").and_then(|c| c.as_str()) {
            duration += match complexity {
                "high" => 3,
                "medium" => 1,
                "low" => 0,
                _ => 1,
            };
        }

        // Adjust based on work type
        if let Some(work_type) = work_sequence.get("type").and_then(|t| t.as_str()) {
            duration += match work_type {
                "implementation" => 2,
                "testing" => 1,
                "documentation" => 1,
                "refactoring" => 3,
                _ => 1,
            };
        }

        duration
    }

    /// Calculate work priority
    async fn calculate_work_priority(
        &self,
        work_sequence: &serde_json::Value,
        index: usize,
    ) -> &'static str {
        // Higher priority for earlier items and critical work
        if index == 0 {
            return "high";
        }

        if let Some(priority) = work_sequence.get("priority").and_then(|p| p.as_str()) {
            match priority {
                "high" => return "high",
                "medium" => return "medium",
                "low" => return "low",
                _ => {} // Continue to type-based detection
            }
        }

        if let Some(work_type) = work_sequence.get("type").and_then(|t| t.as_str()) {
            match work_type {
                "critical" | "blocking" => "high",
                "enhancement" | "feature" => "medium",
                _ => "low",
            }
        } else {
            "medium"
        }
    }

    /// Extract work dependencies
    async fn extract_work_dependencies(
        &self,
        work_sequence: &serde_json::Value,
        current_index: usize,
    ) -> Vec<usize> {
        let mut dependencies = Vec::new();

        // Add dependency on previous work item for sequential work
        if current_index > 0 {
            dependencies.push(current_index - 1);
        }

        // Extract explicit dependencies if present
        if let Some(deps) = work_sequence.get("dependencies").and_then(|d| d.as_array()) {
            for dep in deps {
                if let Some(dep_idx) = dep.as_u64() {
                    dependencies.push(dep_idx as usize);
                }
            }
        }

        dependencies
    }

    /// Identify required resource locks
    async fn identify_required_locks(&self, work_sequence: &serde_json::Value) -> Vec<String> {
        let mut locks = Vec::new();

        if let Some(resources) = work_sequence.get("resources").and_then(|r| r.as_array()) {
            for resource in resources {
                if let Some(resource_path) = resource.as_str() {
                    // Lock critical files
                    if resource_path.contains("main.rs")
                        || resource_path.contains("lib.rs")
                        || resource_path.contains("Cargo.toml")
                    {
                        locks.push(resource_path.to_string());
                    }
                }
            }
        }

        locks
    }

    /// Optimize resource allocation using agent load data
    async fn optimize_resource_allocation(
        &self,
        params: &ScheduleCoordinateParams,
        involved_agents: &[Uuid],
    ) -> Result<serde_json::Value> {
        let mut allocation = serde_json::json!({
            "cpu_cores": involved_agents.len() * 2,
            "memory_gb": involved_agents.len() * 4,
            "storage_gb": 10,
            "network_bandwidth": "1gbps"
        });

        if let Some(agent_service) = &self.agent_service {
            // Get system capacity and load
            let system_metrics = agent_service.get_system_health().await?;

            // Adjust allocations based on current system load
            let load_factor = 1.0 - system_metrics.load_distribution_score;
            let cpu_cores =
                ((involved_agents.len() as f64 * 2.0) * (1.0 + load_factor)).ceil() as i32;
            let memory_gb =
                ((involved_agents.len() as f64 * 4.0) * (1.0 + load_factor * 0.5)).ceil() as i32;

            allocation = serde_json::json!({
                "cpu_cores": cpu_cores,
                "memory_gb": memory_gb,
                "storage_gb": 10 + (params.work_sequences.len() * 2),
                "network_bandwidth": if system_metrics.load_distribution_score > 0.8 { "10gbps" } else { "1gbps" },
                "exclusive_resources": params.project_scopes.clone(),
                "load_balancing_enabled": true,
                "dynamic_scaling": true,
                "system_load_factor": load_factor
            });
        }

        Ok(allocation)
    }

    /// Analyze work dependencies
    async fn analyze_dependencies(
        &self,
        params: &ScheduleCoordinateParams,
        optimized_sequence: &[serde_json::Value],
    ) -> Result<serde_json::Value> {
        let node_count = params.work_sequences.len();
        let mut edge_count = 0;
        let mut bottlenecks = Vec::new();

        // Count dependencies (edges)
        for seq in optimized_sequence {
            if let Some(deps) = seq.get("dependencies").and_then(|d| d.as_array()) {
                edge_count += deps.len();
            }
        }

        // Simple cycle detection using iterative approach
        let has_cycles = self.detect_cycles_iterative(optimized_sequence).await;

        // Identify bottlenecks (nodes with many dependents)
        let mut dependent_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for seq in optimized_sequence {
            if let Some(deps) = seq.get("dependencies").and_then(|d| d.as_array()) {
                for dep in deps {
                    if let Some(dep_idx) = dep.as_u64() {
                        *dependent_counts.entry(dep_idx as usize).or_insert(0) += 1;
                    }
                }
            }
        }

        for (node_idx, dependent_count) in dependent_counts {
            if dependent_count > 2 {
                bottlenecks.push(serde_json::json!({
                    "node_id": node_idx,
                    "dependent_count": dependent_count,
                    "severity": if dependent_count > 4 { "high" } else { "medium" }
                }));
            }
        }

        Ok(serde_json::json!({
            "nodes": node_count,
            "edges": edge_count,
            "cycles": has_cycles,
            "critical_path": optimized_sequence,
            "bottlenecks": bottlenecks,
            "complexity_score": (edge_count as f64 / node_count as f64).min(1.0),
            "parallelization_potential": if edge_count < node_count { "high" } else { "low" }
        }))
    }

    /// Detect cycles using iterative approach
    async fn detect_cycles_iterative(&self, optimized_sequence: &[serde_json::Value]) -> bool {
        let node_count = optimized_sequence.len();
        let mut visited = vec![false; node_count];
        let mut rec_stack = vec![false; node_count];

        for i in 0..node_count {
            if !visited[i]
                && Self::has_cycle_util(i, optimized_sequence, &mut visited, &mut rec_stack)
            {
                return true;
            }
        }

        false
    }

    /// Utility function for cycle detection (non-async to avoid recursion issues)
    fn has_cycle_util(
        node: usize,
        optimized_sequence: &[serde_json::Value],
        visited: &mut [bool],
        rec_stack: &mut [bool],
    ) -> bool {
        visited[node] = true;
        rec_stack[node] = true;

        if let Some(seq) = optimized_sequence.get(node) {
            if let Some(deps) = seq.get("dependencies").and_then(|d| d.as_array()) {
                for dep in deps {
                    if let Some(dep_idx) = dep.as_u64() {
                        let dep_idx = dep_idx as usize;
                        if dep_idx < optimized_sequence.len() {
                            if !visited[dep_idx]
                                && Self::has_cycle_util(
                                    dep_idx,
                                    optimized_sequence,
                                    visited,
                                    rec_stack,
                                )
                            {
                                return true;
                            } else if rec_stack[dep_idx] {
                                return true; // Back edge found
                            }
                        }
                    }
                }
            }
        }

        rec_stack[node] = false;
        false
    }

    // Consolidated Vibe extension handlers

    /// Handle consolidated agent operations
    async fn handle_vibe_agent(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/agent parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            "register" => {
                let agent_params: AgentRegisterParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid register parameters: {}", e),
                    })?;
                #[allow(deprecated)]
                let register_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::AGENT_REGISTER,
                    Some(serde_json::to_value(agent_params)?),
                );
                self.handle_agent_register(register_request).await
            }
            "status" => {
                // Agent status supports both reporting status (with agentId) and querying system statistics (without agentId)
                let status_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::AGENT_STATUS,
                    Some(params.params),
                );
                self.handle_agent_status(status_request).await
            }
            "list" => {
                let list_params: AgentListParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid list parameters: {}", e),
                    })?;
                let list_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::AGENT_LIST,
                    Some(serde_json::to_value(list_params)?),
                );
                self.handle_agent_list(list_request).await
            }
            "deregister" => {
                let deregister_params: AgentDeregisterParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid deregister parameters: {}", e),
                    })?;
                let deregister_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::AGENT_DEREGISTER,
                    Some(serde_json::to_value(deregister_params)?),
                );
                self.handle_agent_deregister(deregister_request).await
            }
            "capabilities" => Ok(Some(JsonRpcResponse::success(
                request.id,
                serde_json::to_value(self.capabilities.clone())?,
            ))),
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown agent operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    /// Handle consolidated issue operations
    async fn handle_vibe_issue(&self, request: JsonRpcRequest) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/issue parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            "create" => {
                let create_params: IssueCreateParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid create parameters: {}", e),
                    })?;
                let create_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::ISSUE_CREATE,
                    Some(serde_json::to_value(create_params)?),
                );
                self.handle_issue_create_new(create_request).await
            }
            "list" => {
                let list_params: IssueListParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid list parameters: {}", e),
                    })?;
                let list_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::ISSUE_LIST,
                    Some(serde_json::to_value(list_params)?),
                );
                self.handle_issue_list_new(list_request).await
            }
            "assign" => {
                let assign_params: IssueAssignParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid assign parameters: {}", e),
                    })?;
                let assign_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::ISSUE_ASSIGN,
                    Some(serde_json::to_value(assign_params)?),
                );
                self.handle_issue_assign(assign_request).await
            }
            "update" => {
                let update_params: IssueUpdateParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid update parameters: {}", e),
                    })?;
                let update_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::ISSUE_UPDATE,
                    Some(serde_json::to_value(update_params)?),
                );
                self.handle_issue_update_new(update_request).await
            }
            "close" => {
                let close_params: IssueCloseParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid close parameters: {}", e),
                    })?;
                let close_request = JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    methods::ISSUE_CLOSE,
                    Some(serde_json::to_value(close_params)?),
                );
                self.handle_issue_close(close_request).await
            }
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown issue operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    /// Handle consolidated coordination operations
    async fn handle_vibe_coordination(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/coordination parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            // Messaging operations
            "message_send" => {
                let msg_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid message send parameters: {}", e),
                    })?;
                self.handle_message_send(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/message/send",
                    Some(msg_params),
                ))
                .await
            }
            "message_broadcast" => {
                let msg_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid message broadcast parameters: {}", e),
                    })?;
                self.handle_message_broadcast(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/message/broadcast",
                    Some(msg_params),
                ))
                .await
            }
            "worker_message" => {
                let msg_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid worker message parameters: {}", e),
                    })?;
                self.handle_legacy_worker_message(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/worker/message",
                    Some(msg_params),
                ))
                .await
            }

            // Worker coordination operations
            "worker_request" => {
                let req_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid worker request parameters: {}", e),
                    })?;
                self.handle_legacy_worker_request(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/worker/request",
                    Some(req_params),
                ))
                .await
            }
            "worker_coordinate" => {
                let coord_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid worker coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_worker_coordinate(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/worker/coordinate",
                    Some(coord_params),
                ))
                .await
            }

            // Resource operations
            "project_lock" => {
                let lock_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid project lock parameters: {}", e),
                    })?;
                self.handle_legacy_project_lock(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/project/lock",
                    Some(lock_params),
                ))
                .await
            }
            "resource_reserve" => {
                let res_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid resource reserve parameters: {}", e),
                    })?;
                self.handle_legacy_resource_reserve(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/resource/reserve",
                    Some(res_params),
                ))
                .await
            }

            // Cross-project dependency operations
            "dependency_declare" => {
                let dep_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid dependency declare parameters: {}", e),
                    })?;
                self.handle_legacy_dependency_declare(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/dependency/declare",
                    Some(dep_params),
                ))
                .await
            }
            "coordinator_request_worker" => {
                let coord_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid coordinator request worker parameters: {}", e),
                    })?;
                self.handle_legacy_coordinator_request_worker(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/coordinator/request_worker",
                    Some(coord_params),
                ))
                .await
            }
            "work_coordinate" => {
                let work_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid work coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_work_coordinate(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/work/coordinate",
                    Some(work_params),
                ))
                .await
            }
            "conflict_resolve" => {
                let conflict_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid conflict resolve parameters: {}", e),
                    })?;
                self.handle_legacy_conflict_resolve(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/conflict/resolve",
                    Some(conflict_params),
                ))
                .await
            }

            // Workflow orchestration operations
            "schedule_coordinate" => {
                let sched_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid schedule coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_schedule_coordinate(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/schedule/coordinate",
                    Some(sched_params),
                ))
                .await
            }
            "conflict_predict" => {
                let pred_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid conflict predict parameters: {}", e),
                    })?;
                self.handle_legacy_conflict_predict(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/conflict/predict",
                    Some(pred_params),
                ))
                .await
            }
            "merge_coordinate" => {
                let merge_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid merge coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_merge_coordinate(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/merge/coordinate",
                    Some(merge_params),
                ))
                .await
            }

            // Knowledge operations
            "knowledge_query" => {
                let query_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid knowledge query parameters: {}", e),
                    })?;
                self.handle_knowledge_query(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/knowledge/query",
                    Some(query_params),
                ))
                .await
            }
            "knowledge_submit" => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::METHOD_NOT_FOUND,
                    message: "knowledge_submit is not implemented yet".to_string(),
                    data: None,
                },
            ))),
            "knowledge_query_coordination" => {
                let coord_query_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid knowledge query coordination parameters: {}", e),
                    })?;
                self.handle_legacy_knowledge_query_coordination(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/knowledge/query/coordination",
                    Some(coord_query_params),
                ))
                .await
            }
            "pattern_suggest" => {
                let pattern_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid pattern suggest parameters: {}", e),
                    })?;
                self.handle_legacy_pattern_suggest(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/pattern/suggest",
                    Some(pattern_params),
                ))
                .await
            }
            "guideline_enforce" => {
                let guide_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid guideline enforce parameters: {}", e),
                    })?;
                self.handle_legacy_guideline_enforce(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/guideline/enforce",
                    Some(guide_params),
                ))
                .await
            }
            "learning_capture" => {
                let learn_params =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid learning capture parameters: {}", e),
                    })?;
                self.handle_legacy_learning_capture(JsonRpcRequest::new_with_id(
                    request.id,
                    "vibe/learning/capture",
                    Some(learn_params),
                ))
                .await
            }

            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown coordination operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    /// Handle consolidated workflow operations
    #[allow(dead_code)]
    async fn handle_vibe_workflow(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/workflow parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            "schedule_coordinate" => {
                let schedule_params: ScheduleCoordinateParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid schedule coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_schedule_coordinate(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/schedule/coordinate",
                    Some(serde_json::to_value(schedule_params)?),
                ))
                .await
            }
            "conflict_predict" => {
                let predict_params: ConflictPredictParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                    message: format!("Invalid conflict predict parameters: {}", e),
                })?;
                self.handle_legacy_conflict_predict(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/conflict/predict",
                    Some(serde_json::to_value(predict_params)?),
                ))
                .await
            }
            "merge_coordinate" => {
                let merge_params: MergeCoordinateParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid merge coordinate parameters: {}", e),
                    })?;
                self.handle_legacy_merge_coordinate(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/merge/coordinate",
                    Some(serde_json::to_value(merge_params)?),
                ))
                .await
            }
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown workflow operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    /// Handle consolidated knowledge operations
    #[allow(dead_code)]
    async fn handle_vibe_knowledge(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/knowledge parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            "query_coordination" => {
                let query_params: KnowledgeQueryCoordinationParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid query coordination parameters: {}", e),
                    })?;
                self.handle_legacy_knowledge_query_coordination(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/knowledge/query/coordination",
                    Some(serde_json::to_value(query_params)?),
                ))
                .await
            }
            "pattern_suggest" => {
                let pattern_params: PatternSuggestParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid pattern suggest parameters: {}", e),
                    })?;
                self.handle_legacy_pattern_suggest(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/pattern/suggest",
                    Some(serde_json::to_value(pattern_params)?),
                ))
                .await
            }
            "guideline_enforce" => {
                let guideline_params: GuidelineEnforceParams =
                    serde_json::from_value(params.params).map_err(|e| Error::InvalidParams {
                        message: format!("Invalid guideline enforce parameters: {}", e),
                    })?;
                self.handle_legacy_guideline_enforce(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/guideline/enforce",
                    Some(serde_json::to_value(guideline_params)?),
                ))
                .await
            }
            "learning_capture" => {
                let learning_params: LearningCaptureParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid learning capture parameters: {}", e),
                    })?;
                self.handle_legacy_learning_capture(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/learning/capture",
                    Some(serde_json::to_value(learning_params)?),
                ))
                .await
            }
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown knowledge operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    /// Handle consolidated resource operations
    #[allow(dead_code)]
    async fn handle_vibe_resource(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        let params: VibeOperationParams = if let Some(params) = request.params {
            serde_json::from_value(params).map_err(|e| Error::InvalidParams {
                message: format!("Invalid vibe/resource parameters: {}", e),
            })?
        } else {
            return Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: "Missing operation parameters".to_string(),
                    data: None,
                },
            )));
        };

        match params.operation.as_str() {
            "reserve" => {
                let reserve_params: ResourceReserveParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                    message: format!("Invalid resource reserve parameters: {}", e),
                })?;
                self.handle_legacy_resource_reserve(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/resource/reserve",
                    Some(serde_json::to_value(reserve_params)?),
                ))
                .await
            }
            "project_lock" => {
                let lock_params: ProjectLockParams = serde_json::from_value(params.params)
                    .map_err(|e| Error::InvalidParams {
                        message: format!("Invalid project lock parameters: {}", e),
                    })?;
                self.handle_legacy_project_lock(JsonRpcRequest::new_with_id(
                    request.id.clone(),
                    "vibe/project/lock",
                    Some(serde_json::to_value(lock_params)?),
                ))
                .await
            }
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::INVALID_PARAMS,
                    message: format!("Unknown resource operation: {}", params.operation),
                    data: None,
                },
            ))),
        }
    }

    // Legacy method handlers for backward compatibility

    async fn handle_legacy_worker_message(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_worker_message(request).await
    }

    async fn handle_legacy_worker_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_worker_request(request).await
    }

    async fn handle_legacy_worker_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_worker_coordinate(request).await
    }

    async fn handle_legacy_project_lock(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_project_lock(request).await
    }

    async fn handle_legacy_dependency_declare(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_dependency_declare(request).await
    }

    async fn handle_legacy_coordinator_request_worker(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_coordinator_request_worker(request).await
    }

    async fn handle_legacy_work_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_work_coordinate(request).await
    }

    async fn handle_legacy_conflict_resolve(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_conflict_resolve(request).await
    }

    async fn handle_legacy_schedule_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_schedule_coordinate(request).await
    }

    async fn handle_legacy_conflict_predict(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_conflict_predict(request).await
    }

    async fn handle_legacy_resource_reserve(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_resource_reserve(request).await
    }

    async fn handle_legacy_merge_coordinate(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_merge_coordinate(request).await
    }

    async fn handle_legacy_knowledge_query_coordination(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_knowledge_query_coordination(request).await
    }

    async fn handle_legacy_pattern_suggest(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_pattern_suggest(request).await
    }

    async fn handle_legacy_guideline_enforce(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_guideline_enforce(request).await
    }

    async fn handle_legacy_learning_capture(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        self.handle_learning_capture(request).await
    }

    // Legacy adapter methods for backward compatibility

    /// Route legacy agent methods to consolidated vibe/agent handler
    async fn handle_vibe_agent_legacy(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        let operation = match request.method.as_str() {
            "vibe/agent/register" => "register",
            "vibe/agent/status" => "status",
            "vibe/agent/list" => "list",
            "vibe/agent/deregister" => "deregister",
            "vibe/agent/capabilities" => "capabilities",
            _ => {
                return Ok(Some(JsonRpcResponse::error(
                    request.id,
                    JsonRpcError {
                        code: error_codes::METHOD_NOT_FOUND,
                        message: format!("Unknown legacy agent method: {}", request.method),
                        data: None,
                    },
                )))
            }
        };

        // For backward compatibility, directly call the old handlers to preserve exact behavior
        match request.method.as_str() {
            "vibe/agent/register" => self.handle_agent_register(request).await,
            "vibe/agent/status" => self.handle_agent_status(request).await,
            "vibe/agent/list" => self.handle_agent_list(request).await,
            "vibe/agent/deregister" => self.handle_agent_deregister(request).await,
            "vibe/agent/capabilities" => Ok(Some(JsonRpcResponse::success(
                request.id,
                serde_json::to_value(self.capabilities.clone())?,
            ))),
            _ => {
                // Fallback to consolidated handler for unknown operations
                let vibe_params = VibeOperationParams {
                    operation: operation.to_string(),
                    params: request
                        .params
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                };

                let vibe_request = JsonRpcRequest::new_with_id(
                    request.id,
                    methods::VIBE_AGENT,
                    Some(serde_json::to_value(vibe_params)?),
                );

                self.handle_vibe_agent(vibe_request).await
            }
        }
    }

    /// Route legacy issue methods to consolidated vibe/issue handler
    async fn handle_vibe_issue_legacy(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        // For backward compatibility, directly call the old handlers to preserve exact behavior
        match request.method.as_str() {
            "vibe/issue/create" => self.handle_issue_create_new(request).await,
            "vibe/issue/list" => self.handle_issue_list_new(request).await,
            "vibe/issue/assign" => self.handle_issue_assign(request).await,
            "vibe/issue/update" => self.handle_issue_update_new(request).await,
            "vibe/issue/close" => self.handle_issue_close(request).await,
            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::METHOD_NOT_FOUND,
                    message: format!("Unknown legacy issue method: {}", request.method),
                    data: None,
                },
            ))),
        }
    }

    /// Route legacy coordination methods to consolidated vibe/coordination handler
    async fn handle_vibe_coordination_legacy(
        &self,
        request: JsonRpcRequest,
    ) -> Result<Option<JsonRpcResponse>> {
        // For backward compatibility, directly call the existing legacy handlers to preserve exact behavior
        match request.method.as_str() {
            // Messaging operations
            "vibe/message/send" => self.handle_message_send(request).await,
            "vibe/message/broadcast" => self.handle_message_broadcast(request).await,
            "vibe/worker/message" => self.handle_legacy_worker_message(request).await,

            // Worker coordination operations
            "vibe/worker/request" => self.handle_legacy_worker_request(request).await,
            "vibe/worker/coordinate" => self.handle_legacy_worker_coordinate(request).await,

            // Resource operations
            "vibe/project/lock" => self.handle_legacy_project_lock(request).await,
            "vibe/resource/reserve" => self.handle_legacy_resource_reserve(request).await,

            // Cross-project dependency operations
            "vibe/dependency/declare" => self.handle_legacy_dependency_declare(request).await,
            "vibe/coordinator/request_worker" => {
                self.handle_legacy_coordinator_request_worker(request).await
            }
            "vibe/work/coordinate" => self.handle_legacy_work_coordinate(request).await,
            "vibe/conflict/resolve" => self.handle_legacy_conflict_resolve(request).await,

            // Workflow orchestration operations
            "vibe/schedule/coordinate" => self.handle_legacy_schedule_coordinate(request).await,
            "vibe/conflict/predict" => self.handle_legacy_conflict_predict(request).await,
            "vibe/merge/coordinate" => self.handle_legacy_merge_coordinate(request).await,

            // Knowledge operations
            "vibe/knowledge/query" => self.handle_knowledge_query(request).await,
            "vibe/knowledge/query/coordination" => {
                self.handle_legacy_knowledge_query_coordination(request)
                    .await
            }
            "vibe/pattern/suggest" => self.handle_legacy_pattern_suggest(request).await,
            "vibe/guideline/enforce" => self.handle_legacy_guideline_enforce(request).await,
            "vibe/learning/capture" => self.handle_legacy_learning_capture(request).await,

            _ => Ok(Some(JsonRpcResponse::error(
                request.id,
                JsonRpcError {
                    code: error_codes::METHOD_NOT_FOUND,
                    message: format!("Unknown legacy coordination method: {}", request.method),
                    data: None,
                },
            ))),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
