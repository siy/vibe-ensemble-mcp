use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::Value;
use tracing::{debug, error, info, trace, warn};

use super::{
    bidirectional_tools::*, client_tools::*, dependency_tools::*, event_tools::*,
    integration_tools::*, orchestration_tools::*, permission_tools::*, project_tools::*,
    ticket_tools::*, tools::ToolRegistry, types::*, worker_type_tools::*, MCP_PROTOCOL_VERSION,
};
use crate::{config::Config, error::Result, server::AppState};

pub struct McpServer {
    pub tools: ToolRegistry,
}

impl Default for McpServer {
    fn default() -> Self {
        // Create a default config with WebSocket enabled
        let config = Config {
            database_path: String::new(),
            host: String::new(),
            port: 0,
            no_respawn: false,
            permission_mode: crate::permissions::PermissionMode::Inherit,
            client_tool_timeout_secs: 30,
            max_concurrent_client_requests: 50,
            sse_echo_allowlist: std::collections::HashSet::new(),
        };
        Self::new(&config)
    }
}

/// Macro to register multiple tools at once
macro_rules! register_tools {
    ($registry:expr, $($tool:expr),+ $(,)?) => {
        $(
            $registry.register($tool);
        )+
    };
}

impl McpServer {
    pub fn new(_config: &Config) -> Self {
        let mut tools = ToolRegistry::new();

        Self::register_project_tools(&mut tools);
        Self::register_ticket_tools(&mut tools);
        Self::register_event_tools(&mut tools);
        Self::register_permission_tools(&mut tools);

        // Always register WebSocket tools (WebSocket is always enabled)
        Self::register_websocket_tools(&mut tools);

        Self { tools }
    }

    /// Register project and worker type management tools
    fn register_project_tools(tools: &mut ToolRegistry) {
        register_tools!(
            tools,
            // Project management tools
            CreateProjectTool,
            ListProjectsTool,
            GetProjectTool,
            UpdateProjectTool,
            DeleteProjectTool,
            // Worker type management tools
            CreateWorkerTypeTool,
            ListWorkerTypesTool,
            GetWorkerTypeTool,
            UpdateWorkerTypeTool,
            DeleteWorkerTypeTool,
        );
    }

    /// Register ticket and dependency management tools
    fn register_ticket_tools(tools: &mut ToolRegistry) {
        register_tools!(
            tools,
            // Ticket management tools
            CreateTicketTool,
            GetTicketTool,
            ListTicketsTool,
            AddTicketCommentTool,
            CloseTicketTool,
            ResumeTicketProcessingTool,
            // Dependency management tools
            AddTicketDependencyTool,
            RemoveTicketDependencyTool,
            GetDependencyGraphTool,
            ListReadyTicketsTool,
            ListBlockedTicketsTool,
        );
    }

    /// Register event and stage management tools
    fn register_event_tools(tools: &mut ToolRegistry) {
        register_tools!(
            tools,
            ListEventsTool,
            ResolveEventTool,
            GetTicketsByStageTool,
        );
    }

    /// Register permission management tools
    fn register_permission_tools(tools: &mut ToolRegistry) {
        register_tools!(tools, GetPermissionModelTool,);
    }

    /// Register WebSocket and bidirectional communication tools
    fn register_websocket_tools(tools: &mut ToolRegistry) {
        register_tools!(
            tools,
            // Client tools for bidirectional communication
            ListClientToolsTool,
            CallClientToolTool,
            ListConnectedClientsTool,
            ListPendingRequestsTool,
            // Orchestration tools for complex workflows
            ExecuteWorkflowTool,
            ParallelCallTool,
            BroadcastToClientsTool,
            // Enhanced bidirectional MCP tools
            CollaborativeSyncTool,
            PollClientStatusTool,
            ClientGroupManagerTool,
            ClientHealthMonitorTool,
            // Integration testing and compatibility tools
            ValidateWebSocketIntegrationTool,
            TestWebSocketCompatibilityTool,
        );
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
            "tools/list" => {
                // Check if this is a paginated request by looking for params
                if request.params.is_some() {
                    self.handle_list_tools_with_pagination(request.params).await
                } else {
                    self.handle_list_tools().await
                }
            }
            "tools/call" => self.handle_call_tool(state, request.params).await,
            "prompts/list" => self.handle_list_prompts().await,
            "prompts/get" => self.handle_get_prompt(request.params).await,
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
        let server_supported_version = MCP_PROTOCOL_VERSION;

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
                prompts: PromptsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "vibe-ensemble-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
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
        self.handle_list_tools_with_pagination(None).await
    }

    async fn handle_list_tools_with_pagination(
        &self,
        params: Option<Value>,
    ) -> std::result::Result<Value, JsonRpcError> {
        info!("Handling list_tools request with pagination");

        // Parse pagination parameters if provided
        let pagination_params = if let Some(params) = params {
            serde_json::from_value::<PaginationParams>(params).map_err(|e| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Invalid pagination params: {}", e),
                data: None,
            })?
        } else {
            PaginationParams { cursor: None }
        };

        // Parse cursor
        let cursor =
            PaginationCursor::from_cursor_string(pagination_params.cursor).map_err(|e| {
                JsonRpcError {
                    code: INVALID_PARAMS,
                    message: format!("Invalid cursor: {}", e),
                    data: None,
                }
            })?;

        // Get all tools and apply pagination
        let all_tools = self.tools.list_tools();
        let total_tools = all_tools.len();

        let start = cursor.offset;
        let end = std::cmp::min(start + cursor.page_size, total_tools);
        let has_more = end < total_tools;

        let paginated_tools = if start >= total_tools {
            Vec::new()
        } else {
            all_tools[start..end].to_vec()
        };

        // Generate next cursor if there are more results
        let next_cursor = if has_more {
            cursor.next_cursor(true)
        } else {
            None
        };

        let response = ListToolsResponse {
            tools: paginated_tools,
            next_cursor,
        };

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

        // Log parameters if they exist and are not empty
        if let Some(ref args) = request.arguments {
            let should_log = match args {
                Value::Null => false,
                Value::Object(map) => !map.is_empty(),
                _ => true,
            };
            if should_log {
                info!(
                    "Tool parameters: {}",
                    serde_json::to_string_pretty(args)
                        .unwrap_or_else(|_| "Failed to serialize parameters".to_string())
                );
            }
        }

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

    async fn handle_list_prompts(&self) -> std::result::Result<Value, JsonRpcError> {
        info!("Handling list_prompts request");

        let prompts = vec![
            Prompt {
                name: "vibe-ensemble-overview".to_string(),
                description: "Comprehensive overview of the Vibe Ensemble MCP server capabilities, tools, and how to use them effectively for multi-agent coordination".to_string(),
                arguments: vec![],
            },
            Prompt {
                name: "project-setup".to_string(),
                description: "Step-by-step guide for setting up a new project with worker types and initial configuration".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "project_name".to_string(),
                        description: "Name of the project to set up".to_string(),
                        required: true,
                    }
                ],
            },
            Prompt {
                name: "multi-agent-workflow".to_string(),
                description: "Best practices and examples for coordinating multiple agents on complex tasks".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "task_type".to_string(),
                        description: "Type of task (development, analysis, testing, etc.)".to_string(),
                        required: false,
                    }
                ],
            },
        ];

        let response = ListPromptsResponse {
            prompts,
            next_cursor: None,
        };

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize prompts: {}", e),
            data: None,
        })?;

        Ok(result)
    }

    async fn handle_get_prompt(
        &self,
        params: Option<Value>,
    ) -> std::result::Result<Value, JsonRpcError> {
        let request: GetPromptRequest = match params {
            Some(params) => serde_json::from_value(params).map_err(|e| JsonRpcError {
                code: INVALID_PARAMS,
                message: format!("Invalid get_prompt params: {}", e),
                data: None,
            })?,
            None => {
                return Err(JsonRpcError {
                    code: INVALID_PARAMS,
                    message: "Missing get_prompt parameters".to_string(),
                    data: None,
                })
            }
        };

        info!("Getting prompt: {}", request.name);

        let messages = match request.name.as_str() {
            "vibe-ensemble-overview" => vec![PromptMessage {
                role: "user".to_string(),
                content: PromptContent {
                    content_type: "text".to_string(),
                    text: include_str!("../../templates/prompts/vibe-ensemble-overview.md")
                        .to_string(),
                },
            }],
            "project-setup" => {
                let project_name = request
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("project_name"))
                    .and_then(|name| name.as_str())
                    .unwrap_or("my-project");

                let template = include_str!("../../templates/prompts/project-setup.md");
                vec![PromptMessage {
                    role: "user".to_string(),
                    content: PromptContent {
                        content_type: "text".to_string(),
                        text: template.replace("{project_name}", project_name),
                    },
                }]
            }
            "multi-agent-workflow" => {
                let task_type = request
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("task_type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("development");

                let template = include_str!("../../templates/prompts/multi-agent-workflow.md");
                vec![PromptMessage {
                    role: "user".to_string(),
                    content: PromptContent {
                        content_type: "text".to_string(),
                        text: template.replace("{task_type}", task_type),
                    },
                }]
            }
            _ => {
                return Err(JsonRpcError {
                    code: INVALID_PARAMS,
                    message: format!("Unknown prompt: {}", request.name),
                    data: None,
                })
            }
        };

        let response = GetPromptResponse { messages };

        let result = serde_json::to_value(response).map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: format!("Failed to serialize prompt response: {}", e),
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
    trace!(
        "MCP request received: {}",
        serde_json::to_string_pretty(&request)
            .unwrap_or_else(|_| "Failed to serialize request".to_string())
    );

    // Check for MCP-Protocol-Version header (2025-06-18 spec requirement)
    if let Some(header_version) = headers.get("MCP-Protocol-Version") {
        if let Ok(version_str) = header_version.to_str() {
            info!("MCP-Protocol-Version header received: {}", version_str);

            // Validate the header version matches what we support
            if version_str != MCP_PROTOCOL_VERSION {
                warn!(
                    "MCP-Protocol-Version header mismatch: client sent {}, server supports {}",
                    version_str, MCP_PROTOCOL_VERSION
                );
            }
        } else {
            warn!("Invalid MCP-Protocol-Version header value");
        }
    } else {
        debug!("No MCP-Protocol-Version header present (optional for HTTP transport)");
    }

    let response = state.mcp_server.handle_request(&state, request).await;

    trace!(
        "MCP response: {}",
        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Failed to serialize response".to_string())
    );

    Ok(Json(response))
}
