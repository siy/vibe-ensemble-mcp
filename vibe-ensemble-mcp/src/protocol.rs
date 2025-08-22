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

/// MCP specific method names
pub mod methods {
    // Standard MCP protocol methods
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "initialized";
    pub const PING: &str = "ping";
    pub const LIST_TOOLS: &str = "tools/list";
    pub const CALL_TOOL: &str = "tools/call";
    pub const LIST_RESOURCES: &str = "resources/list";
    pub const GET_RESOURCE: &str = "resources/get";
    pub const LIST_PROMPTS: &str = "prompts/list";
    pub const GET_PROMPT: &str = "prompts/get";

    // Vibe Ensemble extensions
    pub const AGENT_REGISTER: &str = "vibe/agent/register";
    pub const AGENT_STATUS: &str = "vibe/agent/status";
    pub const AGENT_LIST: &str = "vibe/agent/list";
    pub const AGENT_DEREGISTER: &str = "vibe/agent/deregister";
    pub const AGENT_CAPABILITIES: &str = "vibe/agent/capabilities";
    pub const ISSUE_CREATE: &str = "vibe/issue/create";
    pub const ISSUE_LIST: &str = "vibe/issue/list";
    pub const ISSUE_ASSIGN: &str = "vibe/issue/assign";
    pub const ISSUE_UPDATE: &str = "vibe/issue/update";
    pub const ISSUE_CLOSE: &str = "vibe/issue/close";
    pub const MESSAGE_SEND: &str = "vibe/message/send";
    pub const MESSAGE_BROADCAST: &str = "vibe/message/broadcast";
    pub const KNOWLEDGE_QUERY: &str = "vibe/knowledge/query";
    pub const KNOWLEDGE_SUBMIT: &str = "vibe/knowledge/submit";
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
    #[serde(rename = "agentId")]
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
}
