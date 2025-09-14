use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::Value;
use tracing::{debug, error, info, trace, warn};

use super::{
    event_tools::*, project_tools::*, ticket_tools::*, tools::ToolRegistry, types::*,
    worker_tools::*, worker_type_tools::*,
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
        tools.register(ListWorkersTool);
        tools.register(FinishWorkerTool);

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
        tools.register(UpdateTicketStageTool);
        tools.register(CloseTicketTool);
        tools.register(ClaimTicketTool);
        tools.register(ReleaseTicketTool);

        // Register event and stage management tools
        tools.register(ListEventsTool);
        tools.register(GetTicketsByStageTool);

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
                version: "0.5.1".to_string(),
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
- THEN: Spawn workers using `spawn_worker` with specific queue assignments
- Workers pull tasks from their designated queues automatically
- Monitor worker status with `get_worker_status` and `list_workers`

### 3. Ticketing System
- Create tickets with execution plans using `create_ticket`
- Tickets have multi-stage execution with progress tracking
- Assign tickets to appropriate queues for worker processing
- Monitor progress through comments and stage updates

### 4. Task Queues
- Create specialized queues (e.g., 'development', 'testing', 'review')
- Workers are assigned to specific queues when spawned
- Use `assign_task` to route tickets to the appropriate queue
- Monitor queue status and progress

## Available Tools (22 total)

**Project Management**: create_project, list_projects, get_project, update_project, delete_project
**Worker Management**: spawn_worker (now requires queue_name), stop_worker, list_workers, get_worker_status  
**Tickets**: create_ticket, get_ticket, list_tickets, add_ticket_comment, update_ticket_stage, close_ticket
**Queues**: create_queue, list_queues, get_queue_status, delete_queue
**Events**: list_events, get_task_queue, assign_task

## CRITICAL WORKFLOW SEQUENCE
1. **Setup Phase**: Create project → Define worker types
2. **Infrastructure Phase**: Create queues → Spawn workers with queue assignments  
3. **Execution Phase**: Create tickets → Assign to queues → Monitor progress

## Best Practices
- Always define worker types BEFORE creating tickets
- Assign workers to specific queues (development, testing, review, etc.)
- Workers automatically pull from their assigned queues
- Use descriptive queue names that match your workflow stages
- Monitor progress through events and worker status checks

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
- Use `assign_task` to route tickets to appropriate queues

### Step 4: Assign Tasks to Queues (Workers Auto-Spawn)
**⚠️ CRITICAL: Workers are now AUTO-SPAWNED when tasks are assigned!**

Simply assign tasks to appropriate queue names:
- **\"architect-queue\"**: For design and planning tickets
- **\"developer-queue\"**: For implementation tickets
- **\"tester-queue\"**: For validation tickets
- **\"reviewer-queue\"**: For code review tickets
- **\"docs-queue\"**: For documentation tickets

The system automatically:
- Detects if a worker exists for the queue
- Spawns a new worker if needed
- Workers stop when their queue becomes empty

### Step 5: Monitor and Coordinate (Your Only Direct Actions)
- Use `list_events` to track progress
- Use `get_queue_status` to monitor queues  
- Use `get_worker_status` to check worker health
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
4. **ASSIGN TO QUEUES**: Using assign_task (workers auto-spawn)
5. **MONITOR PROGRESS**: Using list_events, get_queue_status, get_worker_status

## Key Success Factors
1. **Worker types MUST exist before creating tickets**
2. **Workers AUTO-SPAWN when tasks are assigned to queues**  
3. **Simply assign tasks to queue names (e.g., \"architect-queue\", \"developer-queue\")**
4. **No need to manually create queues or spawn workers**
5. **Workers automatically pull from their designated queue and stop when empty**
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
4. ✅ Assign tickets to queues (queues/workers auto-managed on assignment)

## Queue-Based Coordination Strategy

### 1. Task Queue Architecture
**For {} Tasks, organize work into specialized queues:**
- **analysis-queue**: Requirements analysis, dependency mapping
- **development-queue**: Feature implementation, coding
- **testing-queue**: Validation, QA, automated testing
- **review-queue**: Code reviews, optimization
- **documentation-queue**: Docs, guides, README updates

### 2. Auto-Spawn Worker Pattern
Workers are automatically spawned when tasks are assigned to queues:
```
assign_task(ticket_id, \"analysis-queue\")    # Auto-spawns analyzer worker if needed
assign_task(ticket_id, \"development-queue\") # Auto-spawns implementer worker if needed  
assign_task(ticket_id, \"testing-queue\")     # Auto-spawns tester worker if needed
assign_task(ticket_id, \"review-queue\")      # Auto-spawns reviewer worker if needed
```

Workers automatically pull tasks from their assigned queue and stop when queue becomes empty.

### 3. Ticket-to-Queue Assignment Flow
1. **Coordinator**: Create ticket with multi-stage execution plan
2. **Coordinator**: Assign ticket to first queue: `assign_task(ticket_id, \"analysis-queue\")`
3. **Analyzer Worker**: Automatically picks up task, completes analysis stage
4. **Analyzer Worker**: Adds detailed report via `add_ticket_comment`
5. **Coordinator**: Moves ticket to next queue: `assign_task(ticket_id, \"development-queue\")`
6. **Developer Worker**: Continues from analysis, implements features
7. **Repeat** through all stages until completion

### 4. Queue-Aware Communication Protocol
- Workers use `get_queue_tasks(queue_name)` to get their tasks
- Workers use `add_ticket_comment` with stage reports
- Workers use `complete_ticket_stage` when stage is done
- Coordinator uses `get_queue_status` to monitor queue loads
- Coordinator uses `list_events` to track overall progress

### 5. Multi-Stage Handoff Best Practices
- **Clear Stage Boundaries**: Each stage has specific deliverables
- **Queue-Based Routing**: Tickets move between queues, not directly to workers
- **Detailed Handoff Reports**: Workers document their work for next stage
- **Coordinator Oversight**: Review progress before moving to next queue

### 6. Quality & Context Control
- Each worker specializes in their queue's task type only
- Workers validate previous stage work when starting
- All context preserved in ticket comments and stage updates
- Coordinator maintains overall project vision and queue orchestration
- Use `get_worker_status` to ensure workers are healthy and active

### 7. Queue Load Balancing
- Monitor queue status: `get_queue_status(queue_name)`
- Workers auto-spawn when tasks are assigned to queues
- Workers automatically pull next available task from their queue
- Workers stop automatically when their queue becomes empty

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
- Assign tickets to appropriate queues (workers auto-spawn as needed)
- Coordinate workflow between specialized workers
- Ensure proper handoffs between stages

**REMEMBER: Even seemingly simple tasks like \\\"create a README\\\" or \\\"set up initial files\\\" should be delegated to workers through tickets. Your job is pure orchestration.**

This queue-based delegation approach prevents context drift, enables parallel processing, maintains clear separation of concerns, and ensures the coordinator stays focused on workflow management rather than task execution.", task_type, task_type),
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

    trace!(
        "MCP response: {}",
        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Failed to serialize response".to_string())
    );

    Ok(Json(response))
}
