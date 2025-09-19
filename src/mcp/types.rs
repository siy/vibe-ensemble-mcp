use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeRequest {
    #[serde(rename = "protocolVersion", alias = "protocol_version")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo", alias = "client_info")]
    pub client_info: ClientInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged", alias = "list_changed", default)]
    pub list_changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResponse {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
    pub prompts: PromptsCapability,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged", alias = "list_changed", default)]
    pub list_changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema", alias = "input_schema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListToolsResponse {
    pub tools: Vec<Tool>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    pub arguments: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolResponse {
    pub content: Vec<ToolContent>,
    #[serde(
        rename = "isError",
        alias = "is_error",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// Prompt-related types
#[derive(Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPromptsResponse {
    pub prompts: Vec<Prompt>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPromptRequest {
    pub name: String,
    pub arguments: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPromptResponse {
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: PromptContent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

// MCP Error Codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// Pagination types and utilities
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationParams {
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationCursor {
    pub offset: usize,
    pub page_size: usize,
}

impl PaginationCursor {
    pub fn new(offset: usize, page_size: usize) -> Self {
        Self { offset, page_size }
    }

    pub fn from_cursor_string(cursor: Option<String>) -> Result<Self, String> {
        match cursor {
            None => Ok(Self::new(0, 50)), // Default page size of 50
            Some(cursor_str) => {
                use base64::{engine::general_purpose, Engine};
                let decoded = general_purpose::STANDARD
                    .decode(&cursor_str)
                    .map_err(|_| "Invalid cursor format".to_string())?;
                let cursor_json = String::from_utf8(decoded)
                    .map_err(|_| "Invalid cursor encoding".to_string())?;
                let cursor: PaginationCursor = serde_json::from_str(&cursor_json)
                    .map_err(|_| "Invalid cursor structure".to_string())?;
                Ok(cursor)
            }
        }
    }

    pub fn to_cursor_string(&self) -> Result<String, String> {
        use base64::{engine::general_purpose, Engine};
        let cursor_json =
            serde_json::to_string(self).map_err(|_| "Failed to serialize cursor".to_string())?;
        let encoded = general_purpose::STANDARD.encode(cursor_json.as_bytes());
        Ok(encoded)
    }

    pub fn next_cursor(&self, has_more: bool) -> Option<String> {
        if has_more {
            let next = Self::new(self.offset + self.page_size, self.page_size);
            next.to_cursor_string().ok()
        } else {
            None
        }
    }

    /// Apply pagination to a collection and return paginated data with metadata
    pub fn paginate<T: Clone>(&self, items: Vec<T>) -> PaginationResult<T> {
        let total = items.len();
        let start = self.offset;
        let end = std::cmp::min(start + self.page_size, total);
        let has_more = end < total;

        let paginated_items = if start >= total {
            Vec::new()
        } else {
            items[start..end].to_vec()
        };

        let next_cursor = self.next_cursor(has_more);

        PaginationResult {
            items: paginated_items,
            total,
            has_more,
            next_cursor,
        }
    }
}

/// Result of pagination operation with metadata
#[derive(Debug, Clone)]
pub struct PaginationResult<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}
