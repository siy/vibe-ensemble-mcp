use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::Value;
use tracing::{debug, error, info, trace, warn};

use super::{
    event_tools::*, permission_tools::*, project_tools::*, ticket_tools::*, tools::ToolRegistry,
    types::*, worker_type_tools::*,
};
use crate::{error::Result, server::AppState};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

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

        // Worker management is handled automatically by the queue system

        // Register worker type management tools
        tools.register(CreateWorkerTypeTool);
        tools.register(ListWorkerTypesTool);
        tools.register(GetWorkerTypeTool);
        tools.register(UpdateWorkerTypeTool);
        tools.register(DeleteWorkerTypeTool);

        // Register ticket management tools
        tools.register(CreateTicketTool);
        tools.register(GetTicketTool);
        tools.register(ListTicketsTool);
        tools.register(AddTicketCommentTool);
        tools.register(CloseTicketTool);
        tools.register(ResumeTicketProcessingTool);

        // Register event and stage management tools
        tools.register(ListEventsTool);
        tools.register(ResolveEventTool);
        tools.register(GetTicketsByStageTool);

        // Register permission management tools
        tools.register(GetPermissionModelTool);

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
- FIRST: Define specialized worker types with custom system prompts
- THEN: Workers are automatically spawned when tickets are assigned to stages
- Workers pull tasks from their designated stages automatically
- Monitor system status with `list_events` and `get_tickets_by_stage`

### 3. Ticketing System
- Create tickets with execution plans using `create_ticket`
- Tickets have multi-stage execution with progress tracking
- Assign tickets to appropriate queues for worker processing
- Monitor progress through comments and stage updates

### 4. Task Queues
- Create specialized queues (e.g., 'development', 'testing', 'review')
- Workers are assigned to specific queues when spawned
- Tickets automatically advance through stages as workers complete their tasks
- Monitor stage progress with `get_tickets_by_stage` and `list_events`

## Available Tools (20 total)

**Project Management**: create_project, list_projects, get_project, update_project, delete_project
**Worker Types**: create_worker_type, list_worker_types, get_worker_type, update_worker_type, delete_worker_type
**Tickets**: create_ticket, get_ticket, list_tickets, add_ticket_comment, close_ticket, resume_ticket_processing
**Events**: list_events, resolve_event, get_tickets_by_stage
**Permissions**: get_permission_model

## CRITICAL WORKFLOW SEQUENCE
1. **Setup Phase**: Create project → Define worker types with specialized system prompts
2. **Execution Phase**: Create tickets → Workers are automatically spawned when needed  
3. **Monitoring Phase**: Monitor progress through events and ticket comments

## Best Practices
- Always define worker types BEFORE creating tickets
- Workers are automatically spawned by the queue system when tickets are assigned
- Workers automatically process tickets based on their specialized role
- Use descriptive worker type names that match your workflow stages
- Monitor progress through events and worker status checks
- Resolve system events with `resolve_event` after investigation

## 🚨 CRITICAL COORDINATOR PRINCIPLE: DELEGATE EVERYTHING - NO EXCEPTIONS

**⚠️ ABSOLUTE RULE: COORDINATORS MUST NEVER PERFORM TECHNICAL WORK ⚠️**

**As the coordinator, your role is ORCHESTRATION ONLY - you are FORBIDDEN from execution:**

### ❌ NEVER DO THESE (Create Tickets Instead):
- **Writing any code** (even simple scripts, configs, or one-liners)
- **Analyzing requirements** (create analysis tickets)
- **Reading or examining files** (delegate to analysis workers)  
- **Setting up projects** (create setup tickets)
- **Installing dependencies** (create setup tickets)
- **Creating directories or files** (create setup tickets)
- **Debugging issues** (create debugging tickets)
- **Testing features** (create testing tickets)
- **Writing documentation** (create documentation tickets)
- **Researching solutions** (create research tickets)
- **Configuring tools** (create configuration tickets)
- **ANY hands-on technical task** (create appropriate tickets)

### ✅ ONLY DO THESE (Pure Orchestration):
- Create projects and define worker types with system prompts
- Create tickets for ALL work (no matter how simple)
- Assign tickets to appropriate queues (workers auto-spawn)
- Monitor progress through events and queue status
- Coordinate handoffs between specialized workers

**REMEMBER: Even tasks that seem 'too simple to delegate' like 'create a README' or 'make a folder' MUST be delegated through tickets. Your job is PURE ORCHESTRATION - let workers handle 100% of actual work execution.**

## 🔐 PERMISSION ISSUES AND COORDINATOR RESPONSE

When workers encounter permission restrictions and report them via 'CoordinatorAttention' outcome:

### ✅ COORDINATOR MUST:
1. **Call `get_permission_model`** to understand the current permission configuration
2. **Communicate with the user** about the specific tool the worker needs
3. **Explain what the tool does** and why the worker needs it for their task
4. **Ask user for approval** to add the tool to the allowed permissions
5. **Guide user** on which file to edit based on permission model response
6. **Wait for user confirmation** before proceeding

### 📋 PERMISSION TROUBLESHOOTING WORKFLOW:
1. Worker reports: \"CoordinatorAttention: Need access to tool 'WebSearch' to research API documentation\"
2. Coordinator calls `get_permission_model` to understand permission setup
3. Coordinator tells user: \"Worker needs WebSearch tool to research APIs. Current mode is 'inherit' - you need to add 'WebSearch' to .claude/settings.local.json allow array\"
4. User updates permissions and confirms
5. Coordinator instructs user to restart worker or resume ticket processing

### ❌ NEVER DO:
- Ignore permission issues from workers
- Assume user knows how to fix permissions 
- Proceed without user approval for tool access
- Modify permission files yourself (delegate to user)

The system prevents context drift by allowing each worker to focus on their specialty while you (the coordinator) manage the overall workflow through queue-based task distribution and delegation.".to_string(),
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

## CRITICAL: Follow This Exact Sequence

### Step 1: Create the Project
```
Use: create_project
- repository_name: \"{}\"
- path: \"/path/to/your/project\"
- short_description: \"Brief description of your project\"
```

### Step 2: Define Worker Types FIRST (Essential!)
**MUST BE DONE BEFORE CREATING TICKETS**

Define specialized worker types with custom system prompts:
- **analyzer**: Reviews code, identifies issues, suggests improvements
- **implementer**: Writes code, implements features  
- **tester**: Creates and runs tests, validates functionality
- **documenter**: Writes documentation, updates README files
- **reviewer**: Performs code reviews, ensures quality

Each worker type needs its own system prompt tailored to its specialization.

### Step 3: Create Tickets with Execution Plans
**After worker types are defined (workers auto-spawn when tasks are assigned):**
- Break work into tickets with 3-5 stages
- Each stage should specify which worker type handles it
- Include clear success criteria
- Use `update_ticket_stage` to route tickets to appropriate stages

### Step 4: Update Ticket Stages (Workers Auto-Spawn)
**⚠️ CRITICAL: Workers are now AUTO-SPAWNED when tickets reach specific stages!**

Simply update tickets to appropriate stage names:
- **\"planning\"**: For design and architecture work
- **\"coding\"**: For implementation work
- **\"testing\"**: For validation and QA work
- **\"reviewing\"**: For code review work
- **\"documentation\"**: For documentation work

The system automatically:
- Detects if a worker exists for the stage
- Spawns a new worker if needed based on worker types
- Workers stop when their stage work is complete

### Step 5: Monitor and Coordinate (Your Only Direct Actions)
- Use `list_events` to track progress and system notifications
- Use `get_tickets_by_stage` to monitor stage workload
- Use `list_tickets` to check overall progress
- Coordinate handoffs between specialized agents
- **RESIST** the urge to do tasks yourself - always create tickets instead

## 🚨 ABSOLUTE DELEGATION PRINCIPLES - NO EXCEPTIONS

### ❌ COORDINATORS ARE FORBIDDEN FROM:
1. **WRITING CODE**: Even simple scripts, configs, or one-liners → Create tickets
2. **ANALYZING ANYTHING**: Requirements, files, or issues → Create analysis tickets  
3. **SETTING UP PROJECTS**: Folders, files, or configs → Create setup tickets
4. **READING FILES**: Code, docs, or configs → Create review tickets
5. **INSTALLING THINGS**: Dependencies, tools, or packages → Create setup tickets
6. **DEBUGGING**: Issues, errors, or problems → Create debugging tickets
7. **TESTING**: Features, functions, or code → Create testing tickets
8. **DOCUMENTING**: README, guides, or docs → Create documentation tickets
9. **RESEARCHING**: Solutions, libraries, or approaches → Create research tickets
10. **ANY TECHNICAL WORK**: No matter how trivial → Create appropriate tickets

### ✅ COORDINATORS ONLY DO:
1. **CREATE PROJECTS**: Using create_project tool
2. **DEFINE WORKER TYPES**: Using create_worker_type with system prompts
3. **CREATE TICKETS**: For ALL work (even 'simple' tasks)
4. **UPDATE TICKET STAGES**: Using update_ticket_stage (workers auto-spawn)
5. **MONITOR PROGRESS**: Using list_events, get_tickets_by_stage, list_tickets

## Key Success Factors
1. **Worker types MUST exist before creating tickets**
2. **Workers AUTO-SPAWN when tickets reach specific stages**  
3. **Simply update tickets to stage names (e.g., \"planning\", \"coding\", \"testing\")**
4. **No need to manually create stages or spawn workers**
5. **Workers automatically pull from their designated stage and complete when done**
6. **ALL technical work MUST be delegated through tickets**

This delegation-first approach prevents context drift, ensures specialization, and maintains the coordinator's focus on orchestration rather than execution.", project_name, project_name, project_name),
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
                            text: format!("# Multi-Agent Queue-Based Workflow for {} Tasks

## PREREQUISITE: Proper Setup Sequence
**CRITICAL: Before starting any workflow, ensure:**
1. ✅ Project created
2. ✅ Worker types defined with system prompts
3. ✅ Create tickets with execution plans
4. ✅ Update tickets to stages (stages/workers auto-managed on assignment)

## Stage-Based Coordination Strategy

### 1. Task Stage Architecture
**For {} Tasks, organize work into specialized stages:**
- **analysis**: Requirements analysis, dependency mapping
- **coding**: Feature implementation, coding
- **testing**: Validation, QA, automated testing
- **reviewing**: Code reviews, optimization
- **documentation**: Docs, guides, README updates

### 2. Auto-Spawn Worker Pattern
Workers are automatically spawned when tickets are updated to specific stages:
```
update_ticket_stage(ticket_id, \"analysis\")    # Auto-spawns analyzer worker if needed
update_ticket_stage(ticket_id, \"coding\")     # Auto-spawns implementer worker if needed  
update_ticket_stage(ticket_id, \"testing\")   # Auto-spawns tester worker if needed
update_ticket_stage(ticket_id, \"reviewing\") # Auto-spawns reviewer worker if needed
```

Workers automatically pull tasks from their assigned stage and complete when stage work is done.

### 3. Ticket-to-Stage Assignment Flow
1. **Coordinator**: Create ticket with multi-stage execution plan
2. **Coordinator**: Update ticket to first stage: `update_ticket_stage(ticket_id, \"analysis\")`
3. **Analyzer Worker**: Automatically picks up task, completes analysis stage
4. **Analyzer Worker**: Adds detailed report via `add_ticket_comment`
5. **Coordinator**: Moves ticket to next stage: `update_ticket_stage(ticket_id, \"coding\")`
6. **Developer Worker**: Continues from analysis, implements features
7. **Repeat** through all stages until completion

### 4. Stage-Aware Communication Protocol
- Workers use `get_tickets_by_stage(stage_name)` to get their tasks
- Workers use `add_ticket_comment` with stage reports
- Workers use `update_ticket_stage` when stage is done
- Coordinator uses `get_tickets_by_stage` to monitor stage loads
- Coordinator uses `list_events` to track overall progress
- Coordinator uses `resolve_event` to mark events as resolved with investigation summary

### 5. Multi-Stage Handoff Best Practices
- **Clear Stage Boundaries**: Each stage has specific deliverables
- **Stage-Based Routing**: Tickets move between stages, not directly to workers
- **Detailed Handoff Reports**: Workers document their work for next stage
- **Coordinator Oversight**: Review progress before moving to next stage

### 6. Quality & Context Control
- Each worker specializes in their stage's task type only
- Workers validate previous stage work when starting
- All context preserved in ticket comments and stage updates
- Coordinator maintains overall project vision and stage orchestration
- Use `list_events` to ensure system is healthy and active

### 7. Stage Load Balancing
- Monitor stage status: `get_tickets_by_stage(stage_name)`
- Workers auto-spawn when tickets reach specific stages
- Workers automatically pull next available task from their stage
- Workers stop automatically when their stage work is complete

## 🚨 CRITICAL: COORDINATOR DELEGATION RULES

**As coordinator, you must NEVER directly perform any technical work:**

### ❌ What Coordinators MUST NOT Do:
- Write code or scripts
- Analyze requirements or technical specifications
- Set up project files or configurations
- Debug issues or troubleshoot problems
- Create documentation or README files
- Install dependencies or configure tools
- Test features or run validation
- Review code or provide technical feedback

### ✅ What Coordinators SHOULD Do:
- Create projects and define worker types
- Create tickets for ALL technical tasks (no exceptions)
- Update tickets to appropriate stages (workers auto-spawn as needed)
- Coordinate workflow between specialized workers
- Ensure proper handoffs between stages

**REMEMBER: Even seemingly simple tasks like \\\"create a README\\\" or \\\"set up initial files\\\" should be delegated to workers through tickets. Your job is pure orchestration.**

This stage-based delegation approach prevents context drift, enables parallel processing, maintains clear separation of concerns, and ensures the coordinator stays focused on workflow management rather than task execution.", task_type, task_type),
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

    let mcp_server = McpServer::new();
    let response = mcp_server.handle_request(&state, request).await;

    trace!(
        "MCP response: {}",
        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Failed to serialize response".to_string())
    );

    Ok(Json(response))
}
