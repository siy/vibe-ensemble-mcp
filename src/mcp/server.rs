use axum::{extract::State, response::Json};
use serde_json::Value;
use tracing::{info, error, debug};

use crate::{error::Result, server::AppState};
use super::{
    types::*,
    tools::ToolRegistry,
    project_tools::*,
    worker_tools::*,
    queue_tools::*,
    ticket_tools::*,
    event_tools::*,
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
        
        // Register worker management tools
        tools.register(SpawnWorkerTool);
        tools.register(StopWorkerTool);
        tools.register(ListWorkersTool);
        tools.register(GetWorkerStatusTool);
        
        // Register queue management tools
        tools.register(CreateQueueTool);
        tools.register(ListQueuesTool);
        tools.register(GetQueueStatusTool);
        tools.register(DeleteQueueTool);
        
        // Register ticket management tools
        tools.register(CreateTicketTool);
        tools.register(GetTicketTool);
        tools.register(ListTicketsTool);
        tools.register(AddTicketCommentTool);
        tools.register(UpdateTicketStageTool);
        tools.register(CloseTicketTool);
        
        // Register event and task management tools
        tools.register(ListEventsTool);
        tools.register(GetTaskQueueTool);
        tools.register(AssignTaskTool);
        
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

    async fn handle_initialize(&self, params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
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
                version: "0.5.0".to_string(),
            },
        };

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize response: {}", e),
            data: None,
        })?;

        Ok(result)
    }

    async fn handle_list_tools(&self) -> std::result::Result<Value, JsonRpcError> {
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
    ) -> std::result::Result<Value, JsonRpcError> {
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