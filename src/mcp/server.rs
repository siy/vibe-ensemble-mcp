use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::Value;
use tracing::{debug, error, info, trace, warn};

use super::{
    event_tools::*, project_tools::*, queue_tools::*, ticket_tools::*, tools::ToolRegistry,
    types::*, worker_tools::*,
};
use crate::{error::Result, server::AppState};

pub struct McpServer {
    pub tools: ToolRegistry,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
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
            "notifications/initialized" => self.handle_initialized().await,
            "list_tools" | "tools/list" => self.handle_list_tools().await,
            "call_tool" | "tools/call" => self.handle_call_tool(state, request.params).await,
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

    async fn handle_initialize(
        &self,
        params: Option<Value>,
    ) -> std::result::Result<Value, JsonRpcError> {
        info!("Handling initialize request");

        let request: InitializeRequest = match params {
            Some(params) => serde_json::from_value(params).map_err(|e| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Invalid initialize params: {}", e),
                data: None,
            })?,
            None => {
                return Err(JsonRpcError {
                    code: INVALID_PARAMS,
                    message: "Missing initialize parameters".to_string(),
                    data: None,
                })
            }
        };

        // Log protocol version negotiation
        let client_version = &request.protocol_version;
        let server_supported_version = "2024-11-05";
        
        info!(
            "Protocol version negotiation - Client requested: {}, Server supports: {}", 
            client_version, server_supported_version
        );

        // We accept any client version but return what we actually support
        if client_version != server_supported_version {
            info!(
                "Protocol version mismatch: client requested {}, negotiating down to {}", 
                client_version, server_supported_version
            );
        }

        let response = InitializeResponse {
            protocol_version: server_supported_version.to_string(),
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

    async fn handle_initialized(&self) -> std::result::Result<Value, JsonRpcError> {
        info!("Handling notifications/initialized request");
        
        // The notifications/initialized method requires no response according to MCP spec
        // Return null/empty result to acknowledge
        Ok(Value::Null)
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
            None => {
                return Err(JsonRpcError {
                    code: INVALID_PARAMS,
                    message: "Missing call_tool parameters".to_string(),
                    data: None,
                })
            }
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
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>> {
    trace!("MCP request received: {}", serde_json::to_string_pretty(&request).unwrap_or_else(|_| "Failed to serialize request".to_string()));
    
    // Check for MCP-Protocol-Version header (2025-06-18 spec requirement)
    if let Some(header_version) = headers.get("MCP-Protocol-Version") {
        if let Ok(version_str) = header_version.to_str() {
            info!("MCP-Protocol-Version header received: {}", version_str);
            
            // Validate the header version matches what we support
            if version_str != "2024-11-05" {
                warn!(
                    "MCP-Protocol-Version header mismatch: client sent {}, server supports 2024-11-05", 
                    version_str
                );
            }
        } else {
            warn!("Invalid MCP-Protocol-Version header value");
        }
    } else {
        debug!("No MCP-Protocol-Version header present (optional for HTTP transport)");
    }
    
    let mcp_server = McpServer::new();
    let response = mcp_server.handle_request(&state, request).await;
    
    trace!("MCP response: {}", serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Failed to serialize response".to_string()));
    
    Ok(Json(response))
}
