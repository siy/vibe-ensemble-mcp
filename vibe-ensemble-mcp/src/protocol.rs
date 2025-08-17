//! MCP protocol message definitions and handling

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// MCP protocol version
pub const MCP_VERSION: &str = "1.0.0";

/// Base MCP message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessage {
    pub id: Uuid,
    pub method: String,
    pub params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard MCP methods
pub mod methods {
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "initialized";
    pub const PING: &str = "ping";
    pub const PONG: &str = "pong";
    pub const AGENT_REGISTER: &str = "agent/register";
    pub const AGENT_STATUS: &str = "agent/status";
    pub const ISSUE_CREATE: &str = "issue/create";
    pub const ISSUE_UPDATE: &str = "issue/update";
    pub const MESSAGE_SEND: &str = "message/send";
    pub const KNOWLEDGE_QUERY: &str = "knowledge/query";
}

/// Initialization parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Client capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub agent_management: bool,
    pub issue_tracking: bool,
    pub messaging: bool,
    pub knowledge_access: bool,
}

impl McpMessage {
    /// Create a new request message
    pub fn new_request(method: String, params: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            method,
            params,
            result: None,
            error: None,
        }
    }

    /// Create a new response message
    pub fn new_response(id: Uuid, result: serde_json::Value) -> Self {
        Self {
            id,
            method: String::new(),
            params: serde_json::Value::Null,
            result: Some(result),
            error: None,
        }
    }

    /// Create a new error response
    pub fn new_error(id: Uuid, error: McpError) -> Self {
        Self {
            id,
            method: String::new(),
            params: serde_json::Value::Null,
            result: None,
            error: Some(error),
        }
    }

    /// Check if this is a request message
    pub fn is_request(&self) -> bool {
        !self.method.is_empty() && self.result.is_none() && self.error.is_none()
    }

    /// Check if this is a response message
    pub fn is_response(&self) -> bool {
        self.method.is_empty() && (self.result.is_some() || self.error.is_some())
    }
}