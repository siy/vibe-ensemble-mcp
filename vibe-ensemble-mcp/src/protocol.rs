//! MCP protocol message definitions and handling
//!
//! This module provides the core MCP protocol types and message handling
//! based on JSON-RPC 2.0 specification, compliant with MCP 2024-11-05.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// MCP protocol version supported by this implementation
pub const MCP_VERSION: &str = "2024-11-05";

/// JSON-RPC 2.0 message types for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

/// JSON-RPC 2.0 request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 notification message (no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP specific method names - Streamlined to 8 essential core methods
pub mod methods {
    // Standard MCP protocol methods (5 core methods)
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "initialized";
    pub const PING: &str = "ping";
    pub const LIST_TOOLS: &str = "tools/list";
    pub const CALL_TOOL: &str = "tools/call";
    pub const LIST_RESOURCES: &str = "resources/list";
    /// Read a single resource identified by its URI.
    pub const READ_RESOURCE: &str = "resources/read";
    pub const LIST_PROMPTS: &str = "prompts/list";

    // Streamlined Vibe Ensemble extensions (3 essential methods for all functionality)
    /// Agent operations: register, status, list, deregister, capabilities
    pub const VIBE_AGENT: &str = "vibe/agent";

    /// Issue operations: create, list, assign, update, close
    pub const VIBE_ISSUE: &str = "vibe/issue";

    /// Coordination operations: messaging, workflows, knowledge, resources, dependencies, conflicts
    pub const VIBE_COORDINATION: &str = "vibe/coordination";
}

/// MCP initialization parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Client capabilities for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<serde_json::Value>,
}

/// MCP initialization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Server capabilities for MCP with Vibe Ensemble extensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,

    // Vibe Ensemble specific capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibe_agent_management: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibe_issue_tracking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibe_messaging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibe_knowledge_management: Option<bool>,
}

/// Prompts capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resources capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Tools capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Agent registration parameters for Vibe Ensemble
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterParams {
    pub name: String,
    #[serde(rename = "agentType")]
    pub agent_type: String,
    pub capabilities: Vec<String>,
    #[serde(rename = "connectionMetadata")]
    pub connection_metadata: serde_json::Value,
}

impl AgentRegisterParams {
    /// Create AgentRegisterParams with intelligent defaults for common coordination patterns
    pub fn from_json_with_defaults(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        // Try normal deserialization first
        if let Ok(params) = serde_json::from_value::<AgentRegisterParams>(value.clone()) {
            return Ok(params);
        }

        // If that fails, apply intelligent defaults based on partial data
        let obj = value.as_object().ok_or_else(|| {
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Registration parameters must be an object",
            ))
        })?;

        // Extract or default the name
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "claude-code-coordinator".to_string());

        // Extract or default the agent type
        let agent_type = obj
            .get("agentType")
            .or_else(|| obj.get("agent_type"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Intelligent default based on name pattern
                if name.contains("coordinator") {
                    "Coordinator".to_string()
                } else {
                    "Worker".to_string()
                }
            });

        // Extract or default capabilities
        let capabilities = obj
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_else(|| {
                // Intelligent default based on agent type
                if agent_type == "Coordinator" {
                    vec![
                        "cross_project_coordination".to_string(),
                        "dependency_management".to_string(),
                        "conflict_resolution".to_string(),
                        "resource_allocation".to_string(),
                        "workflow_orchestration".to_string(),
                        "strategic_planning".to_string(),
                        "quality_oversight".to_string(),
                    ]
                } else {
                    vec![
                        "task_execution".to_string(),
                        "code_implementation".to_string(),
                        "testing".to_string(),
                    ]
                }
            });

        // Extract or default connection metadata
        let connection_metadata = obj
            .get("connectionMetadata")
            .or_else(|| obj.get("connection_metadata"))
            .cloned()
            .unwrap_or_else(|| {
                // Intelligent default connection metadata
                let endpoint = format!("system://{}", name);
                serde_json::json!({
                    "endpoint": endpoint,
                    "protocol_version": "2024-11-05"
                })
            });

        Ok(AgentRegisterParams {
            name,
            agent_type,
            capabilities,
            connection_metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_intelligent_registration_defaults_empty() {
        let empty_params = json!({});
        let result = AgentRegisterParams::from_json_with_defaults(empty_params);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert_eq!(params.name, "claude-code-coordinator");
        assert_eq!(params.agent_type, "Coordinator");
        assert!(!params.capabilities.is_empty());
        assert!(params
            .capabilities
            .contains(&"cross_project_coordination".to_string()));
    }

    #[test]
    fn test_intelligent_registration_defaults_partial() {
        let partial_params = json!({
            "name": "test-worker"
        });
        let result = AgentRegisterParams::from_json_with_defaults(partial_params);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert_eq!(params.name, "test-worker");
        assert_eq!(params.agent_type, "Worker");
        assert!(params.capabilities.contains(&"task_execution".to_string()));

        // Verify connection metadata has defaults
        let metadata = params.connection_metadata.as_object().unwrap();
        assert!(metadata.contains_key("endpoint"));
        assert!(metadata.contains_key("protocol_version"));
        assert_eq!(metadata["protocol_version"], "2024-11-05");
    }

    #[test]
    fn test_intelligent_registration_coordinator_detection() {
        let coordinator_params = json!({
            "name": "my-coordinator-agent"
        });
        let result = AgentRegisterParams::from_json_with_defaults(coordinator_params);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert_eq!(params.name, "my-coordinator-agent");
        assert_eq!(params.agent_type, "Coordinator");
        assert!(params
            .capabilities
            .contains(&"conflict_resolution".to_string()));
        assert!(params
            .capabilities
            .contains(&"workflow_orchestration".to_string()));
    }

    #[test]
    fn test_full_registration_passthrough() {
        let full_params = json!({
            "name": "full-agent",
            "agentType": "Worker",
            "capabilities": ["custom_capability"],
            "connectionMetadata": {
                "endpoint": "custom://endpoint",
                "protocol_version": "2024-11-05"
            }
        });
        let result = AgentRegisterParams::from_json_with_defaults(full_params);
        assert!(result.is_ok());

        let params = result.unwrap();
        assert_eq!(params.name, "full-agent");
        assert_eq!(params.agent_type, "Worker");
        assert_eq!(params.capabilities, vec!["custom_capability"]);

        let metadata = params.connection_metadata.as_object().unwrap();
        assert_eq!(metadata["endpoint"], "custom://endpoint");
    }
}

/// Agent registration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterResult {
    #[serde(rename = "agentId")]
    pub agent_id: Uuid,
    pub status: String,
    #[serde(rename = "assignedResources")]
    pub assigned_resources: Vec<String>,
}

/// Agent status reporting parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusParams {
    #[serde(rename = "agentId", alias = "agent_id")]
    pub agent_id: String,
    pub status: String,
    #[serde(rename = "currentTask")]
    pub current_task: Option<String>,
    pub progress: Option<f32>,
    #[serde(rename = "healthMetrics")]
    pub health_metrics: Option<serde_json::Value>,
}

/// Agent list query parameters  
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentListParams {
    pub project: Option<String>,
    pub capability: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "agentType")]
    pub agent_type: Option<String>,
    pub limit: Option<usize>,
}

/// Agent deregistration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDeregisterParams {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "shutdownReason")]
    pub shutdown_reason: Option<String>,
}

/// Agent deregistration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDeregisterResult {
    #[serde(rename = "agentId")]
    pub agent_id: Uuid,
    pub status: String,
    #[serde(rename = "cleanupStatus")]
    pub cleanup_status: String,
}

// Consolidated Vibe extension parameters and results

/// Generic Vibe operation parameters for all consolidated endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeOperationParams {
    /// The specific operation to perform (e.g., "register", "create", "send")
    pub operation: String,
    /// Operation-specific parameters
    pub params: serde_json::Value,
}

/// Generic Vibe operation result for all consolidated endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeOperationResult {
    /// The operation that was performed
    pub operation: String,
    /// Whether the operation succeeded
    pub success: bool,
    /// Operation-specific result data
    pub data: serde_json::Value,
    /// Human-readable message
    pub message: String,
}

// Legacy parameter structures for backward compatibility

// Issue tracking MCP tool parameters and results

/// Issue creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCreateParams {
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
    #[serde(rename = "issueType")]
    pub issue_type: Option<String>,
    #[serde(rename = "projectId")]
    pub project_id: Option<String>,
    #[serde(rename = "createdByAgentId")]
    pub created_by_agent_id: String,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<String>,
}

/// Issue creation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCreateResult {
    #[serde(rename = "issueId")]
    pub issue_id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

/// Issue list query parameters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueListParams {
    #[serde(rename = "projectId")]
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub assignee: Option<String>,
    #[serde(rename = "issueType")]
    pub issue_type: Option<String>,
    pub priority: Option<String>,
    pub labels: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// Issue list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueListResult {
    pub issues: Vec<IssueInfo>,
    pub total: usize,
    #[serde(rename = "filtersApplied")]
    pub filters_applied: IssueListParams,
}

/// Issue information for list results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInfo {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub status: String,
    #[serde(rename = "assignedAgentId")]
    pub assigned_agent_id: Option<Uuid>,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "resolvedAt")]
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub tags: Vec<String>,
    #[serde(rename = "knowledgeLinks")]
    pub knowledge_links: Vec<String>,
    #[serde(rename = "isAssigned")]
    pub is_assigned: bool,
    #[serde(rename = "isTerminal")]
    pub is_terminal: bool,
    #[serde(rename = "ageSeconds")]
    pub age_seconds: i64,
}

/// Issue assignment parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAssignParams {
    #[serde(rename = "issueId")]
    pub issue_id: String,
    #[serde(rename = "assigneeAgentId")]
    pub assignee_agent_id: String,
    #[serde(rename = "assignedByAgentId")]
    pub assigned_by_agent_id: String,
    pub reason: Option<String>,
}

/// Issue assignment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAssignResult {
    #[serde(rename = "issueId")]
    pub issue_id: Uuid,
    #[serde(rename = "assigneeAgentId")]
    pub assignee_agent_id: Uuid,
    #[serde(rename = "assignedByAgentId")]
    pub assigned_by_agent_id: Uuid,
    pub status: String,
    #[serde(rename = "assignedAt")]
    pub assigned_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

/// Issue update parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueUpdateParams {
    #[serde(rename = "issueId")]
    pub issue_id: String,
    pub status: Option<String>,
    pub comment: Option<String>,
    #[serde(rename = "updatedByAgentId")]
    pub updated_by_agent_id: String,
    pub priority: Option<String>,
}

/// Issue update result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueUpdateResult {
    #[serde(rename = "issueId")]
    pub issue_id: Uuid,
    pub status: String,
    pub priority: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "commentAdded")]
    pub comment_added: bool,
    pub message: String,
}

/// Issue close parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCloseParams {
    #[serde(rename = "issueId")]
    pub issue_id: String,
    #[serde(rename = "closedByAgentId")]
    pub closed_by_agent_id: String,
    pub resolution: String,
    #[serde(rename = "closeReason")]
    pub close_reason: Option<String>,
}

/// Issue close result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCloseResult {
    #[serde(rename = "issueId")]
    pub issue_id: Uuid,
    #[serde(rename = "closedByAgentId")]
    pub closed_by_agent_id: Uuid,
    pub status: String,
    pub resolution: String,
    #[serde(rename = "closedAt")]
    pub closed_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::String(Uuid::new_v4().to_string()),
            method: method.to_string(),
            params,
        }
    }

    /// Create a new JSON-RPC request with specific ID
    pub fn new_with_id(
        id: serde_json::Value,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcNotification {
    /// Create a new notification
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            experimental: None,
            logging: None,
            prompts: Some(PromptsCapability {
                list_changed: Some(true),
            }),
            resources: Some(ResourcesCapability {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            vibe_agent_management: Some(true),
            vibe_issue_tracking: Some(true),
            vibe_messaging: Some(true),
            vibe_knowledge_management: Some(true),
        }
    }
}

/// Standard JSON-RPC error codes
pub mod error_codes {
    /// Parse error - Invalid JSON
    pub const PARSE_ERROR: i32 = -32700;
    /// Invalid request - The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;
    /// Method not found - The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid params - Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal error - Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // Vibe Ensemble specific error codes (starting from -32000)
    /// Agent registration failed
    pub const AGENT_REGISTRATION_FAILED: i32 = -32000;
    /// Agent not found
    pub const AGENT_NOT_FOUND: i32 = -32001;
    /// Issue creation failed
    pub const ISSUE_CREATION_FAILED: i32 = -32002;
    /// Knowledge access denied
    pub const KNOWLEDGE_ACCESS_DENIED: i32 = -32003;
    /// Transport error
    pub const TRANSPORT_ERROR: i32 = -32004;
    /// Message delivery failed
    pub const MESSAGE_DELIVERY_FAILED: i32 = -32005;
    /// Resource lock conflict
    pub const RESOURCE_LOCK_CONFLICT: i32 = -32006;
    /// Coordination session failed
    pub const COORDINATION_FAILED: i32 = -32007;
}

// Worker Communication MCP tool parameters and results

/// Worker message parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMessageParams {
    #[serde(rename = "recipientAgentId")]
    pub recipient_agent_id: String,
    #[serde(rename = "messageContent")]
    pub message_content: String,
    #[serde(rename = "messageType")]
    pub message_type: String,
    #[serde(rename = "senderAgentId")]
    pub sender_agent_id: String,
    pub priority: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Worker message result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMessageResult {
    #[serde(rename = "messageId")]
    pub message_id: Uuid,
    #[serde(rename = "recipientAgentId")]
    pub recipient_agent_id: Uuid,
    #[serde(rename = "senderAgentId")]
    pub sender_agent_id: Uuid,
    pub status: String,
    #[serde(rename = "sentAt")]
    pub sent_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "deliveryConfirmation")]
    pub delivery_confirmation: bool,
    pub message: String,
}

/// Worker request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequestParams {
    #[serde(rename = "targetAgentId")]
    pub target_agent_id: String,
    #[serde(rename = "requestType")]
    pub request_type: String,
    #[serde(rename = "requestDetails")]
    pub request_details: serde_json::Value,
    #[serde(rename = "requestedByAgentId")]
    pub requested_by_agent_id: String,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: Option<String>,
}

/// Worker request result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequestResult {
    #[serde(rename = "requestId")]
    pub request_id: Uuid,
    #[serde(rename = "targetAgentId")]
    pub target_agent_id: Uuid,
    #[serde(rename = "requestedByAgentId")]
    pub requested_by_agent_id: Uuid,
    #[serde(rename = "requestType")]
    pub request_type: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub message: String,
}

/// Worker coordination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCoordinateParams {
    #[serde(rename = "coordinationType")]
    pub coordination_type: String,
    #[serde(rename = "involvedAgents")]
    pub involved_agents: Vec<String>,
    pub scope: serde_json::Value, // files/modules/projects
    #[serde(rename = "coordinatorAgentId")]
    pub coordinator_agent_id: String,
    pub details: serde_json::Value,
}

/// Worker coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCoordinateResult {
    #[serde(rename = "coordinationSessionId")]
    pub coordination_session_id: Uuid,
    #[serde(rename = "coordinatorAgentId")]
    pub coordinator_agent_id: Uuid,
    #[serde(rename = "involvedAgents")]
    pub involved_agents: Vec<Uuid>,
    #[serde(rename = "coordinationType")]
    pub coordination_type: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "participantConfirmations")]
    pub participant_confirmations: Vec<String>,
    pub message: String,
}

/// Project lock parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLockParams {
    #[serde(rename = "projectId")]
    pub project_id: Option<String>,
    #[serde(rename = "resourcePath")]
    pub resource_path: String,
    #[serde(rename = "lockType")]
    pub lock_type: String, // Exclusive, Shared, Coordination
    #[serde(rename = "lockHolderAgentId")]
    pub lock_holder_agent_id: String,
    pub duration: Option<i64>, // Duration in seconds
    pub reason: String,
}

/// Project lock result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLockResult {
    #[serde(rename = "lockId")]
    pub lock_id: Uuid,
    #[serde(rename = "projectId")]
    pub project_id: Option<String>,
    #[serde(rename = "resourcePath")]
    pub resource_path: String,
    #[serde(rename = "lockType")]
    pub lock_type: String,
    #[serde(rename = "lockHolderAgentId")]
    pub lock_holder_agent_id: Uuid,
    pub status: String,
    #[serde(rename = "lockedAt")]
    pub locked_at: chrono::DateTime<chrono::Utc>,
    pub expiration: Option<chrono::DateTime<chrono::Utc>>,
    pub message: String,
}

// Cross-Project Dependency Coordination MCP tool parameters and results

/// Dependency declaration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDeclareParams {
    #[serde(rename = "declaringAgentId")]
    pub declaring_agent_id: String,
    #[serde(rename = "sourceProject")]
    pub source_project: String,
    #[serde(rename = "targetProject")]
    pub target_project: String,
    #[serde(rename = "dependencyType")]
    pub dependency_type: String,
    pub description: String,
    pub impact: String,
    pub urgency: String,
    #[serde(rename = "affectedFiles")]
    pub affected_files: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Dependency declaration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDeclareResult {
    #[serde(rename = "dependencyId")]
    pub dependency_id: Uuid,
    #[serde(rename = "coordinationPlan")]
    pub coordination_plan: serde_json::Value,
    #[serde(rename = "requiredActions")]
    pub required_actions: Vec<serde_json::Value>,
    #[serde(rename = "targetProjectActiveWorkers")]
    pub target_project_active_workers: Vec<Uuid>,
    #[serde(rename = "issueCreated")]
    pub issue_created: Option<Uuid>,
    pub status: String,
    #[serde(rename = "estimatedResolutionTime")]
    pub estimated_resolution_time: Option<chrono::DateTime<chrono::Utc>>,
    pub message: String,
}

/// Coordinator worker request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorRequestWorkerParams {
    #[serde(rename = "requestingAgentId")]
    pub requesting_agent_id: String,
    #[serde(rename = "targetProject")]
    pub target_project: String,
    #[serde(rename = "requiredCapabilities")]
    pub required_capabilities: Vec<String>,
    pub priority: String,
    #[serde(rename = "taskDescription")]
    pub task_description: String,
    #[serde(rename = "estimatedDuration")]
    pub estimated_duration: Option<String>,
    #[serde(rename = "contextData")]
    pub context_data: Option<serde_json::Value>,
}

/// Coordinator worker request result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorRequestWorkerResult {
    #[serde(rename = "requestId")]
    pub request_id: Uuid,
    #[serde(rename = "workerAssignmentStatus")]
    pub worker_assignment_status: String,
    #[serde(rename = "estimatedSpawnTime")]
    pub estimated_spawn_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "assignedWorkerId")]
    pub assigned_worker_id: Option<Uuid>,
    #[serde(rename = "capabilityMatch")]
    pub capability_match: f32,
    #[serde(rename = "spawnPlan")]
    pub spawn_plan: Option<serde_json::Value>,
    pub status: String,
    pub message: String,
}

/// Work coordination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkCoordinateParams {
    #[serde(rename = "initiatingAgentId")]
    pub initiating_agent_id: String,
    #[serde(rename = "targetAgentId")]
    pub target_agent_id: String,
    #[serde(rename = "coordinationType")]
    pub coordination_type: String,
    #[serde(rename = "workItems")]
    pub work_items: Vec<serde_json::Value>,
    #[serde(default)]
    pub dependencies: Vec<serde_json::Value>,
    #[serde(rename = "proposedTimeline")]
    pub proposed_timeline: Option<serde_json::Value>,
    #[serde(rename = "resourceRequirements")]
    pub resource_requirements: Option<serde_json::Value>,
}

/// Work coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkCoordinateResult {
    #[serde(rename = "coordinationAgreementId")]
    pub coordination_agreement_id: Uuid,
    #[serde(rename = "negotiatedTimeline")]
    pub negotiated_timeline: serde_json::Value,
    #[serde(rename = "workAssignments")]
    pub work_assignments: Vec<serde_json::Value>,
    #[serde(rename = "coordinationStatus")]
    pub coordination_status: String,
    #[serde(rename = "participantConfirmations")]
    pub participant_confirmations: Vec<Uuid>,
    #[serde(rename = "communicationProtocol")]
    pub communication_protocol: serde_json::Value,
    #[serde(rename = "escalationRules")]
    pub escalation_rules: Vec<serde_json::Value>,
    pub message: String,
}

/// Permission decision parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecideParams {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "decision")]
    pub decision: String, // APPROVE or DENY
    #[serde(rename = "approverAgentId")]
    pub approver_agent_id: String,
    pub comment: Option<String>,
}

/// Permission decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecideResult {
    #[serde(rename = "requestId")]
    pub request_id: Uuid,
    pub status: String,
    pub message: String,
}

/// Conflict resolution parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolveParams {
    #[serde(rename = "affectedAgents")]
    pub affected_agents: Vec<String>,
    #[serde(rename = "conflictedResources")]
    pub conflicted_resources: Vec<String>,
    #[serde(rename = "conflictType")]
    pub conflict_type: String,
    #[serde(rename = "resolutionStrategy")]
    pub resolution_strategy: Option<String>,
    #[serde(rename = "resolverAgentId")]
    pub resolver_agent_id: String,
    #[serde(rename = "conflictEvidence")]
    pub conflict_evidence: Vec<serde_json::Value>,
    pub priority: Option<String>,
}

/// Conflict resolution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolveResult {
    #[serde(rename = "resolutionId")]
    pub resolution_id: Uuid,
    #[serde(rename = "resolutionPlan")]
    pub resolution_plan: serde_json::Value,
    #[serde(rename = "requiredActionsPerAgent")]
    pub required_actions_per_agent: serde_json::Value,
    #[serde(rename = "resolutionStrategy")]
    pub resolution_strategy: String,
    #[serde(rename = "estimatedResolutionTime")]
    pub estimated_resolution_time: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "rollbackPlan")]
    pub rollback_plan: Option<serde_json::Value>,
    #[serde(rename = "coordinatorEscalation")]
    pub coordinator_escalation: bool,
    pub status: String,
    pub message: String,
}

// Issue #52: Intelligent Work Orchestration MCP tool parameters and results

/// Schedule coordination parameters  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleCoordinateParams {
    #[serde(rename = "coordinatorAgentId")]
    pub coordinator_agent_id: String,
    #[serde(rename = "workSequences")]
    pub work_sequences: Vec<serde_json::Value>,
    #[serde(rename = "involvedAgents")]
    pub involved_agents: Vec<String>,
    #[serde(rename = "projectScopes")]
    pub project_scopes: Vec<String>,
    #[serde(rename = "resourceRequirements")]
    pub resource_requirements: serde_json::Value,
    #[serde(rename = "timeConstraints")]
    pub time_constraints: Option<serde_json::Value>,
}

/// Schedule coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleCoordinateResult {
    #[serde(rename = "coordinationScheduleId")]
    pub coordination_schedule_id: Uuid,
    #[serde(rename = "optimizedSequence")]
    pub optimized_sequence: Vec<serde_json::Value>,
    #[serde(rename = "resourceAllocations")]
    pub resource_allocations: serde_json::Value,
    #[serde(rename = "dependencyGraph")]
    pub dependency_graph: serde_json::Value,
    #[serde(rename = "estimatedCompletionTime")]
    pub estimated_completion_time: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "conflictWarnings")]
    pub conflict_warnings: Vec<String>,
    pub status: String,
    pub message: String,
}

/// Conflict prediction parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictPredictParams {
    #[serde(rename = "analyzerAgentId")]
    pub analyzer_agent_id: String,
    #[serde(rename = "plannedActions")]
    pub planned_actions: Vec<serde_json::Value>,
    #[serde(rename = "activeWorkflows")]
    pub active_workflows: Vec<serde_json::Value>,
    #[serde(rename = "resourceMap")]
    pub resource_map: serde_json::Value,
    #[serde(rename = "timeHorizon")]
    pub time_horizon: Option<String>,
}

/// Conflict prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictPredictResult {
    #[serde(rename = "analysisId")]
    pub analysis_id: Uuid,
    #[serde(rename = "predictedConflicts")]
    pub predicted_conflicts: Vec<serde_json::Value>,
    #[serde(rename = "riskAssessment")]
    pub risk_assessment: serde_json::Value,
    #[serde(rename = "recommendedActions")]
    pub recommended_actions: Vec<serde_json::Value>,
    #[serde(rename = "preventionStrategies")]
    pub prevention_strategies: Vec<String>,
    #[serde(rename = "monitoringPoints")]
    pub monitoring_points: Vec<String>,
    pub confidence: f32,
    pub message: String,
}

/// Resource reservation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReserveParams {
    #[serde(rename = "reservingAgentId")]
    pub reserving_agent_id: String,
    #[serde(rename = "resourcePaths")]
    pub resource_paths: Vec<String>,
    #[serde(rename = "reservationType")]
    pub reservation_type: String,
    #[serde(rename = "reservationDuration")]
    pub reservation_duration: String,
    #[serde(rename = "exclusiveAccess")]
    pub exclusive_access: bool,
    #[serde(rename = "allowedOperations")]
    pub allowed_operations: Vec<String>,
    pub justification: String,
}

/// Resource reservation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReserveResult {
    #[serde(rename = "reservationId")]
    pub reservation_id: Uuid,
    #[serde(rename = "reservedResources")]
    pub reserved_resources: Vec<serde_json::Value>,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "expirationTime")]
    pub expiration_time: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "conflictingReservations")]
    pub conflicting_reservations: Vec<Uuid>,
    #[serde(rename = "coordinationRequired")]
    pub coordination_required: bool,
    pub status: String,
    pub message: String,
}

/// Merge coordination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeCoordinateParams {
    #[serde(rename = "coordinatorAgentId")]
    pub coordinator_agent_id: String,
    #[serde(rename = "mergeScenario")]
    pub merge_scenario: String,
    #[serde(rename = "sourceBranches")]
    pub source_branches: Vec<String>,
    #[serde(rename = "targetBranch")]
    pub target_branch: String,
    #[serde(rename = "involvedAgents")]
    pub involved_agents: Vec<String>,
    #[serde(rename = "complexityAnalysis")]
    pub complexity_analysis: serde_json::Value,
    #[serde(rename = "conflictResolutionStrategy")]
    pub conflict_resolution_strategy: Option<String>,
}

/// Merge coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeCoordinateResult {
    #[serde(rename = "mergeCoordinationId")]
    pub merge_coordination_id: Uuid,
    #[serde(rename = "mergeStrategy")]
    pub merge_strategy: String,
    #[serde(rename = "sequencePlan")]
    pub sequence_plan: Vec<serde_json::Value>,
    #[serde(rename = "conflictResolutionPlan")]
    pub conflict_resolution_plan: serde_json::Value,
    #[serde(rename = "reviewAssignments")]
    pub review_assignments: Vec<serde_json::Value>,
    #[serde(rename = "estimatedMergeTime")]
    pub estimated_merge_time: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "rollbackPlan")]
    pub rollback_plan: serde_json::Value,
    pub message: String,
}

// Issue #53: Knowledge-Driven Coordination MCP tool parameters and results

/// Knowledge query for coordination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQueryCoordinationParams {
    #[serde(rename = "queryingAgentId")]
    pub querying_agent_id: String,
    #[serde(rename = "coordinationContext")]
    pub coordination_context: String,
    pub query: String,
    #[serde(rename = "searchScope")]
    pub search_scope: Vec<String>,
    #[serde(rename = "relevanceCriteria")]
    pub relevance_criteria: Option<serde_json::Value>,
    #[serde(rename = "maxResults")]
    pub max_results: Option<i32>,
}

/// Knowledge query for coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQueryCoordinationResult {
    #[serde(rename = "queryId")]
    pub query_id: Uuid,
    #[serde(rename = "relevantPatterns")]
    pub relevant_patterns: Vec<serde_json::Value>,
    #[serde(rename = "bestPractices")]
    pub best_practices: Vec<serde_json::Value>,
    #[serde(rename = "historicalSolutions")]
    pub historical_solutions: Vec<serde_json::Value>,
    #[serde(rename = "organizationalGuidelines")]
    pub organizational_guidelines: Vec<serde_json::Value>,
    #[serde(rename = "confidenceScore")]
    pub confidence_score: f32,
    #[serde(rename = "applicabilityRating")]
    pub applicability_rating: f32,
    pub message: String,
}

/// Pattern suggestion parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSuggestParams {
    #[serde(rename = "requestingAgentId")]
    pub requesting_agent_id: String,
    #[serde(rename = "coordinationScenario")]
    pub coordination_scenario: String,
    #[serde(rename = "currentContext")]
    pub current_context: serde_json::Value,
    #[serde(rename = "similarityThreshold")]
    pub similarity_threshold: Option<f32>,
    #[serde(rename = "excludePatterns")]
    pub exclude_patterns: Vec<String>,
}

/// Pattern suggestion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSuggestResult {
    #[serde(rename = "suggestionId")]
    pub suggestion_id: Uuid,
    #[serde(rename = "recommendedPatterns")]
    pub recommended_patterns: Vec<serde_json::Value>,
    #[serde(rename = "adaptationGuidance")]
    pub adaptation_guidance: Vec<String>,
    #[serde(rename = "implementationSteps")]
    pub implementation_steps: Vec<serde_json::Value>,
    #[serde(rename = "successProbability")]
    pub success_probability: f32,
    #[serde(rename = "alternativeApproaches")]
    pub alternative_approaches: Vec<serde_json::Value>,
    #[serde(rename = "riskFactors")]
    pub risk_factors: Vec<String>,
    pub message: String,
}

/// Guideline enforcement parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidelineEnforceParams {
    #[serde(rename = "enforcingAgentId")]
    pub enforcing_agent_id: String,
    #[serde(rename = "coordinationPlan")]
    pub coordination_plan: serde_json::Value,
    #[serde(rename = "applicableGuidelines")]
    pub applicable_guidelines: Vec<String>,
    #[serde(rename = "enforcementLevel")]
    pub enforcement_level: String,
    #[serde(rename = "allowExceptions")]
    pub allow_exceptions: bool,
}

/// Guideline enforcement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidelineEnforceResult {
    #[serde(rename = "enforcementId")]
    pub enforcement_id: Uuid,
    #[serde(rename = "complianceStatus")]
    pub compliance_status: String,
    #[serde(rename = "violations")]
    pub violations: Vec<serde_json::Value>,
    #[serde(rename = "recommendedCorrections")]
    pub recommended_corrections: Vec<serde_json::Value>,
    #[serde(rename = "approvedExceptions")]
    pub approved_exceptions: Vec<serde_json::Value>,
    #[serde(rename = "complianceScore")]
    pub compliance_score: f32,
    #[serde(rename = "auditTrail")]
    pub audit_trail: Vec<serde_json::Value>,
    pub message: String,
}

/// Learning capture parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningCaptureParams {
    #[serde(rename = "capturingAgentId")]
    pub capturing_agent_id: String,
    #[serde(rename = "coordinationSession")]
    pub coordination_session: serde_json::Value,
    #[serde(rename = "outcomeData")]
    pub outcome_data: serde_json::Value,
    #[serde(rename = "successMetrics")]
    pub success_metrics: serde_json::Value,
    #[serde(rename = "lessonsLearned")]
    pub lessons_learned: Vec<String>,
    #[serde(rename = "improvementOpportunities")]
    pub improvement_opportunities: Vec<String>,
}

/// Learning capture result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningCaptureResult {
    #[serde(rename = "learningRecordId")]
    pub learning_record_id: Uuid,
    #[serde(rename = "extractedPatterns")]
    pub extracted_patterns: Vec<serde_json::Value>,
    #[serde(rename = "knowledgeContributions")]
    pub knowledge_contributions: Vec<serde_json::Value>,
    #[serde(rename = "processImprovements")]
    pub process_improvements: Vec<serde_json::Value>,
    #[serde(rename = "organizationalLearning")]
    pub organizational_learning: serde_json::Value,
    #[serde(rename = "futureRecommendations")]
    pub future_recommendations: Vec<String>,
    #[serde(rename = "knowledgeQualityScore")]
    pub knowledge_quality_score: f32,
    pub message: String,
}
