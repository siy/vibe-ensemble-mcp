# Stage 3: MCP Protocol Implementation

**Duration**: 3-4 hours  
**Goal**: Full HTTP MCP server with basic tools

## Overview

This stage implements the complete Model Context Protocol (MCP) over HTTP, including protocol handlers, tool framework, and basic project/worker type management tools. The server will support the MCP JSON-RPC protocol with proper error handling and response formatting.

## Objectives

1. Implement MCP protocol handlers (initialize, list_tools, call_tool)
2. Create tool framework with parameter validation
3. Implement project management tools (5 tools)
4. Implement worker type management tools (5 tools)
5. Add proper MCP error handling and response formatting
6. Create tool registry and dispatch system

## MCP Protocol Specification

The MCP server implements JSON-RPC 2.0 over HTTP with the following core methods:

### Core Protocol Methods
- `initialize` - Client capability negotiation
- `list_tools` - Return available tools
- `call_tool` - Execute a specific tool

### Tool Categories
1. **Project Management** (5 tools)
2. **Worker Type Management** (5 tools)
3. **Worker Management** (4 tools) - Stage 4
4. **Queue Management** (3 tools) - Stage 4
5. **Ticket Management** (6 tools) - Stage 5
6. **Event Management** (2 tools) - Stage 5

## Implementation

### 1. MCP Protocol Types (`src/mcp/types.rs`)

```rust
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
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(default)]
    pub list_changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResponse {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListToolsResponse {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    pub arguments: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolResponse {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolContent {
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
```

### 2. Tool Framework (`src/mcp/tools.rs`)

```rust
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::{error::Result, server::AppState};
use super::types::{CallToolRequest, CallToolResponse, Tool, ToolContent};

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse>;
    fn definition(&self) -> Tool;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<T: ToolHandler + 'static>(&mut self, tool: T) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, Box::new(tool));
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn ToolHandler>> {
        self.tools.get(name)
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    pub async fn call_tool(
        &self,
        state: &AppState,
        request: CallToolRequest,
    ) -> Result<CallToolResponse> {
        match self.get_tool(&request.name) {
            Some(tool) => tool.call(state, request.arguments).await,
            None => Ok(CallToolResponse {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: format!("Tool '{}' not found", request.name),
                }],
                is_error: Some(true),
            }),
        }
    }
}

pub fn create_success_response(message: &str) -> CallToolResponse {
    CallToolResponse {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: message.to_string(),
        }],
        is_error: None,
    }
}

pub fn create_error_response(error: &str) -> CallToolResponse {
    CallToolResponse {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: error.to_string(),
        }],
        is_error: Some(true),
    }
}

// Utility function to extract and validate parameters
pub fn extract_param<T>(arguments: &Option<Value>, key: &str) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    match arguments {
        Some(Value::Object(map)) => {
            match map.get(key) {
                Some(value) => serde_json::from_value(value.clone())
                    .map_err(|e| crate::error::AppError::BadRequest(
                        format!("Invalid parameter '{}': {}", key, e)
                    )),
                None => Err(crate::error::AppError::BadRequest(
                    format!("Missing required parameter '{}'", key)
                )),
            }
        }
        _ => Err(crate::error::AppError::BadRequest(
            "Arguments must be an object".to_string()
        )),
    }
}

pub fn extract_optional_param<T>(arguments: &Option<Value>, key: &str) -> Result<Option<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    match arguments {
        Some(Value::Object(map)) => {
            match map.get(key) {
                Some(value) if !value.is_null() => {
                    let parsed: T = serde_json::from_value(value.clone())
                        .map_err(|e| crate::error::AppError::BadRequest(
                            format!("Invalid parameter '{}': {}", key, e)
                        ))?;
                    Ok(Some(parsed))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}
```

### 3. Project Management Tools (`src/mcp/project_tools.rs`)

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    database::projects::{Project, CreateProjectRequest, UpdateProjectRequest},
    error::Result,
    server::AppState,
};
use super::tools::{ToolHandler, extract_param, extract_optional_param, create_success_response, create_error_response};
use super::types::{CallToolResponse, Tool};

pub struct CreateProjectTool;

#[async_trait]
impl ToolHandler for CreateProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;
        let path: String = extract_param(&arguments, "path")?;
        let short_description: Option<String> = extract_optional_param(&arguments, "description")?;

        let request = CreateProjectRequest {
            repository_name: repository_name.clone(),
            path,
            short_description,
        };

        match Project::create(&state.db, request).await {
            Ok(project) => {
                let response = json!({
                    "repository_name": project.repository_name,
                    "path": project.path,
                    "description": project.short_description,
                    "created_at": project.created_at
                });
                Ok(create_success_response(&format!("Project created successfully: {}", response)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to create project: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_project".to_string(),
            description: "Create a new project with repository name and path".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    },
                    "path": {
                        "type": "string",
                        "description": "Local path to the project directory"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional short description of the project"
                    }
                },
                "required": ["repository_name", "path"]
            }),
        }
    }
}

pub struct ListProjectsTool;

#[async_trait]
impl ToolHandler for ListProjectsTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        match Project::list_all(&state.db).await {
            Ok(projects) => {
                let projects_json = serde_json::to_string_pretty(&projects)?;
                Ok(create_success_response(&format!("Projects:\n{}", projects_json)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to list projects: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_projects".to_string(),
            description: "List all projects".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

pub struct GetProjectTool;

#[async_trait]
impl ToolHandler for GetProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;

        match Project::get_by_name(&state.db, &repository_name).await {
            Ok(Some(project)) => {
                let project_json = serde_json::to_string_pretty(&project)?;
                Ok(create_success_response(&format!("Project:\n{}", project_json)))
            }
            Ok(None) => Ok(create_error_response(&format!("Project '{}' not found", repository_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to get project: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_project".to_string(),
            description: "Get project details by repository name".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}

pub struct UpdateProjectTool;

#[async_trait]
impl ToolHandler for UpdateProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;
        let path: Option<String> = extract_optional_param(&arguments, "path")?;
        let short_description: Option<String> = extract_optional_param(&arguments, "description")?;

        let request = UpdateProjectRequest {
            path,
            short_description,
        };

        match Project::update(&state.db, &repository_name, request).await {
            Ok(Some(project)) => {
                let project_json = serde_json::to_string_pretty(&project)?;
                Ok(create_success_response(&format!("Project updated:\n{}", project_json)))
            }
            Ok(None) => Ok(create_error_response(&format!("Project '{}' not found", repository_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to update project: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "update_project".to_string(),
            description: "Update project details".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    },
                    "path": {
                        "type": "string",
                        "description": "New path to the project directory"
                    },
                    "description": {
                        "type": "string",
                        "description": "New short description of the project"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}

pub struct DeleteProjectTool;

#[async_trait]
impl ToolHandler for DeleteProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;

        match Project::delete(&state.db, &repository_name).await {
            Ok(true) => Ok(create_success_response(&format!("Project '{}' deleted successfully", repository_name))),
            Ok(false) => Ok(create_error_response(&format!("Project '{}' not found", repository_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to delete project: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_project".to_string(),
            description: "Delete a project by repository name".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}
```

### 4. MCP Server Implementation (`src/mcp/server.rs`)

```rust
use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use tracing::{info, error, debug};

use crate::{error::Result, server::AppState};
use super::{
    types::*,
    tools::ToolRegistry,
    project_tools::*,
};

pub struct McpServer {
    pub tools: ToolRegistry,
}

impl McpServer {
    pub fn new() -> Self {
        let mut tools = ToolRegistry::new();
        
        // Register project management tools
        tools.register(CreateProjectTool);
        tools.register(ListProjectsTool);
        tools.register(GetProjectTool);
        tools.register(UpdateProjectTool);
        tools.register(DeleteProjectTool);
        
        Self { tools }
    }

    pub async fn handle_request(
        &self,
        state: &AppState,
        request: JsonRpcRequest,
    ) -> JsonRpcResponse {
        debug!("Handling MCP request: {}", request.method);

        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "list_tools" => self.handle_list_tools().await,
            "call_tool" => self.handle_call_tool(state, request.params).await,
            _ => Err(JsonRpcError {
                code: METHOD_NOT_FOUND,
                message: format!("Method '{}' not found", request.method),
                data: None,
            }),
        };

        match response {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(error),
            },
        }
    }

    async fn handle_initialize(&self, params: Option<Value>) -> Result<Value, JsonRpcError> {
        info!("Handling initialize request");

        let _request: InitializeRequest = match params {
            Some(params) => serde_json::from_value(params).map_err(|e| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Invalid initialize params: {}", e),
                data: None,
            })?,
            None => return Err(JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing initialize parameters".to_string(),
                data: None,
            }),
        };

        let response = InitializeResponse {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "vibe-ensemble-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize response: {}", e),
            data: None,
        })?;

        Ok(result)
    }

    async fn handle_list_tools(&self) -> Result<Value, JsonRpcError> {
        info!("Handling list_tools request");

        let tools = self.tools.list_tools();
        let response = ListToolsResponse { tools };

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize tools: {}", e),
            data: None,
        })?;

        Ok(result)
    }

    async fn handle_call_tool(
        &self,
        state: &AppState,
        params: Option<Value>,
    ) -> Result<Value, JsonRpcError> {
        let request: CallToolRequest = match params {
            Some(params) => serde_json::from_value(params).map_err(|e| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Invalid call_tool params: {}", e),
                data: None,
            })?,
            None => return Err(JsonRpcError {
                code: INVALID_PARAMS,
                message: "Missing call_tool parameters".to_string(),
                data: None,
            }),
        };

        info!("Calling tool: {}", request.name);

        let response = self.tools.call_tool(state, request).await.map_err(|e| {
            error!("Tool execution error: {}", e);
            JsonRpcError {
                code: INTERNAL_ERROR,
                message: format!("Tool execution failed: {}", e),
                data: None,
            }
        })?;

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize tool response: {}", e),
            data: None,
        })?;

        Ok(result)
    }
}

pub async fn mcp_handler(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>> {
    let mcp_server = McpServer::new();
    let response = mcp_server.handle_request(&state, request).await;
    Ok(Json(response))
}
```

### 5. Integration with Main Server

Update `src/server.rs`:
```rust
use crate::mcp::server::mcp_handler;

// Replace the placeholder mcp_handler with:
.route("/mcp", post(mcp_handler))
```

Update `src/mcp/mod.rs`:
```rust
pub mod types;
pub mod tools;
pub mod server;
pub mod project_tools;
// Future: worker_type_tools, worker_tools, etc.
```

## Testing

### 1. MCP Protocol Test

```bash
# Test initialize
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocol_version": "2024-11-05",
      "capabilities": {"tools": {"list_changed": false}},
      "client_info": {"name": "test-client", "version": "1.0"}
    }
  }'
```

### 2. Tools Test

```bash
# List tools
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "list_tools"
  }'

# Create project
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "call_tool",
    "params": {
      "name": "create_project",
      "arguments": {
        "repository_name": "test/project",
        "path": "/tmp/test",
        "description": "Test project"
      }
    }
  }'
```

## Validation Checklist

- [ ] MCP initialize method works correctly
- [ ] List tools returns all registered tools
- [ ] All project management tools work
- [ ] Proper JSON-RPC error handling
- [ ] Tool parameter validation works
- [ ] Database integration functions correctly
- [ ] Error responses are properly formatted

## Next Steps

After completing Stage 3:
1. Test all MCP tools thoroughly
2. Verify JSON-RPC protocol compliance
3. Update progress in [TODO.md](../TODO.md)
4. Proceed to [Stage 4: Worker Management](STAGE_4_WORKER_MANAGEMENT.md)