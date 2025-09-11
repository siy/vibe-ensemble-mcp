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
            "list_prompts" | "prompts/list" => self.handle_list_prompts().await,
            "get_prompt" | "prompts/get" => self.handle_get_prompt(request.params).await,
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
                prompts: PromptsCapability {
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

        let response = ListPromptsResponse { prompts };

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
            "vibe-ensemble-overview" => vec![
                PromptMessage {
                    role: "user".to_string(),
                    content: PromptContent {
                        content_type: "text".to_string(),
                        text: "You are now connected to the Vibe Ensemble MCP server - a sophisticated multi-agent coordination system. Here's what you need to know:

# Vibe Ensemble Overview

**Purpose**: Coordinate multiple specialized agents to prevent context drift and maintain focus across complex, multi-stage projects.

## Core Concepts

### 1. Projects
- Create projects with `create_project` to establish workspaces
- Each project can have multiple specialized worker types
- Projects track all tickets, workers, and progress

### 2. Worker Types & Agents
- Define specialized worker types with custom system prompts
- Spawn workers using `spawn_worker` for specific tasks
- Workers operate independently but report back to the coordinator
- Monitor worker status with `get_worker_status` and `list_workers`

### 3. Ticketing System
- Create tickets with execution plans using `create_ticket`
- Tickets have multi-stage execution with progress tracking
- Add comments and update stages as work progresses
- Close tickets when complete with `close_ticket`

### 4. Task Queues
- Create queues for organizing work with `create_queue`
- Assign tasks to workers using `assign_task`
- Monitor queue status and progress

## Available Tools (22 total)

**Project Management**: create_project, list_projects, get_project, update_project, delete_project
**Worker Management**: spawn_worker, stop_worker, list_workers, get_worker_status  
**Tickets**: create_ticket, get_ticket, list_tickets, add_ticket_comment, update_ticket_stage, close_ticket
**Queues**: create_queue, list_queues, get_queue_status, delete_queue
**Events**: list_events, get_task_queue, assign_task

## Best Practices
1. Start by creating a project for your workspace
2. Define worker types for different specializations (analysis, implementation, testing, etc.)
3. Create tickets with clear execution plans
4. Use queues to organize and distribute work
5. Monitor progress through events and status checks

The system prevents context drift by allowing each worker to focus on their specialty while the coordinator (you) manages the overall workflow.".to_string(),
                    },
                }
            ],
            "project-setup" => {
                let project_name = request.arguments
                    .as_ref()
                    .and_then(|args| args.get("project_name"))
                    .and_then(|name| name.as_str())
                    .unwrap_or("my-project");

                vec![
                    PromptMessage {
                        role: "user".to_string(),
                        content: PromptContent {
                            content_type: "text".to_string(),
                            text: format!("Here's a step-by-step guide to set up the '{}' project using Vibe Ensemble:

# Project Setup Guide for '{}'

## Step 1: Create the Project
```
Use: create_project
- repository_name: \"{}\"
- path: \"/path/to/your/project\"
- short_description: \"Brief description of your project\"
```

## Step 2: Define Worker Types
Consider these common worker specializations:
- **analyzer**: Reviews code, identifies issues, suggests improvements
- **implementer**: Writes code, implements features
- **tester**: Creates and runs tests, validates functionality
- **documenter**: Writes documentation, updates README files
- **reviewer**: Performs code reviews, ensures quality

## Step 3: Create Initial Tickets
Break down your work into tickets with clear execution plans:
- Each ticket should have 3-5 stages
- Assign appropriate worker types to each stage
- Include success criteria

## Step 4: Set Up Queues
Organize work with task queues:
- Priority queue for urgent tasks
- Development queue for feature work
- Bug queue for fixes
- Review queue for code reviews

## Step 5: Start Multi-Agent Coordination
1. Spawn workers as needed
2. Assign tickets to appropriate workers
3. Monitor progress through events
4. Coordinate handoffs between agents

This setup enables efficient multi-agent collaboration while preventing context drift.", project_name, project_name, project_name),
                        },
                    }
                ]
            }
            "multi-agent-workflow" => {
                let task_type = request.arguments
                    .as_ref()
                    .and_then(|args| args.get("task_type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("development");

                vec![
                    PromptMessage {
                        role: "user".to_string(),
                        content: PromptContent {
                            content_type: "text".to_string(),
                            text: format!("# Multi-Agent Workflow for {} Tasks

## Coordination Strategy

### 1. Task Decomposition
Break complex tasks into stages that match worker specializations:

**For {} Tasks:**
- Analysis: Understanding requirements, identifying dependencies
- Planning: Creating detailed execution plans
- Implementation: Writing code, building features
- Testing: Validation, quality assurance
- Review: Code review, optimization
- Documentation: Updating docs, writing guides

### 2. Agent Handoff Pattern
1. **Coordinator** (you): Create ticket with execution plan
2. **Specialist Agent**: Complete their stage, update progress
3. **Coordinator**: Review progress, assign next stage
4. **Next Specialist**: Continue from previous work
5. **Repeat** until ticket completion

### 3. Communication Protocol
- Use `add_ticket_comment` for detailed progress reports
- Update ticket stages with `update_ticket_stage`
- Monitor all agents with `list_events`
- Check worker status regularly

### 4. Quality Control
- Each stage should have clear acceptance criteria
- Workers should validate previous stage work
- Use comments to communicate issues or blockers
- Coordinator reviews all stage transitions

### 5. Context Preservation
- Workers focus only on their specialized tasks
- All context stored in tickets and comments
- No single agent carries full project context
- Coordinator maintains overall project vision

This approach prevents context drift while enabling deep specialization.", task_type, task_type),
                        },
                    }
                ]
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
