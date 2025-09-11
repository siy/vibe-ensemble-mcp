# Stage 4: Worker Management

**Duration**: 3-4 hours  
**Goal**: Complete worker lifecycle and queue management

## Overview

This stage implements the complete worker management system, including process spawning for Claude Code headless workers, status tracking, health monitoring, and in-memory queue management. Workers will be spawned as separate Claude Code processes with dedicated task queues.

## Objectives

1. Implement worker process spawning and management
2. Create worker status tracking and health monitoring
3. Build in-memory queue system with worker-queue binding
4. Add worker lifecycle management tools (4 tools)
5. Implement queue management tools (3 tools)
6. Create task distribution and worker communication system

## Architecture

```
┌─────────────────┐    HTTP MCP    ┌──────────────────┐
│   Coordinator   │◄──────────────►│  MCP Server      │
│ (Claude Code)   │                │                  │
└─────────────────┘                │  ┌─────────────┐ │    stdio    ┌─────────────┐
                                   │  │   Queue     │ │◄───────────►│   Worker 1  │
                   spawn_worker     │  │  Manager    │ │             │ (Headless)  │
                   ────────────────►│  │             │ │    stdio    ├─────────────┤
                                   │  │  ┌───────┐  │ │◄───────────►│   Worker 2  │
                                   │  │  │Queue 1│  │ │             │ (Headless)  │
                                   │  │  │Queue 2│  │ │             └─────────────┘
                                   │  │  │Queue N│  │ │
                                   │  │  └───────┘  │ │
                                   │  └─────────────┘ │
                                   └──────────────────┘
```

## Implementation

### 1. Worker Types and Status (`src/workers/types.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Child};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerStatus {
    Spawning,
    Active,
    Idle,
    Finished,
    Failed,
}

impl WorkerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerStatus::Spawning => "spawning",
            WorkerStatus::Active => "active", 
            WorkerStatus::Idle => "idle",
            WorkerStatus::Finished => "finished",
            WorkerStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub project_id: String,
    pub worker_type: String,
    pub status: WorkerStatus,
    pub pid: Option<u32>,
    pub queue_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub struct WorkerProcess {
    pub info: WorkerInfo,
    pub process: Option<Child>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnWorkerRequest {
    pub worker_id: String,
    pub project_id: String,
    pub worker_type: String,
}

pub type WorkerRegistry = RwLock<HashMap<String, WorkerProcess>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub task_id: String,
    pub ticket_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub type TaskQueue = RwLock<Vec<TaskItem>>;
pub type QueueRegistry = RwLock<HashMap<String, TaskQueue>>;
```

### 2. Worker Process Manager (`src/workers/process.rs`)

```rust
use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use tokio::{process::Child, time::Duration};
use tracing::{info, error, warn, debug};

use crate::{
    database::{worker_types::WorkerType, workers::Worker},
    server::AppState,
};
use super::types::{WorkerInfo, WorkerProcess, WorkerStatus, SpawnWorkerRequest};

pub struct ProcessManager;

impl ProcessManager {
    pub async fn spawn_worker(
        state: &AppState,
        request: SpawnWorkerRequest,
    ) -> Result<WorkerProcess> {
        info!("Spawning worker: {}", request.worker_id);

        // Get project info
        let project = crate::database::projects::Project::get_by_name(
            &state.db,
            &request.project_id
        ).await?
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", request.project_id))?;

        // Get worker type info
        let worker_type_info = WorkerType::get_by_type(
            &state.db,
            &request.project_id,
            &request.worker_type,
        ).await?
        .ok_or_else(|| anyhow::anyhow!(
            "Worker type '{}' not found for project '{}'",
            request.worker_type,
            request.project_id
        ))?;

        // Generate queue name
        let queue_name = request.worker_id.replace("worker_", "queue_");

        // Create worker info
        let now = chrono::Utc::now();
        let worker_info = WorkerInfo {
            worker_id: request.worker_id.clone(),
            project_id: request.project_id.clone(),
            worker_type: request.worker_type.clone(),
            status: WorkerStatus::Spawning,
            pid: None,
            queue_name: queue_name.clone(),
            started_at: now,
            last_activity: now,
        };

        // Save worker to database
        let db_worker = Worker {
            worker_id: worker_info.worker_id.clone(),
            project_id: worker_info.project_id.clone(),
            worker_type: worker_info.worker_type.clone(),
            status: worker_info.status.as_str().to_string(),
            pid: None,
            queue_name: worker_info.queue_name.clone(),
            started_at: worker_info.started_at.to_rfc3339(),
            last_activity: worker_info.last_activity.to_rfc3339(),
        };
        Worker::create(&state.db, db_worker).await?;

        // Build worker prompt
        let worker_prompt = Self::build_worker_prompt(
            &worker_info,
            &worker_type_info.system_prompt,
            &queue_name,
        );

        // Spawn Claude Code process
        let mut cmd = Command::new("claude");
        cmd.arg("-p")
           .arg(&worker_prompt)
           .arg("--database-path")
           .arg(&format!("{}/worker_{}.db", project.path, worker_info.worker_id))
           .current_dir(&project.path)
           .stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .kill_on_drop(true);

        debug!("Executing command: {:?}", cmd);

        let child = tokio::process::Command::from(cmd)
            .spawn()
            .context("Failed to spawn Claude Code process")?;

        let pid = child.id();
        info!("Worker {} spawned with PID: {:?}", worker_info.worker_id, pid);

        // Update worker info with PID and status
        let mut updated_info = worker_info.clone();
        updated_info.pid = pid;
        updated_info.status = WorkerStatus::Active;

        // Update database
        Worker::update_status(&state.db, &updated_info.worker_id, "active", pid).await?;

        Ok(WorkerProcess {
            info: updated_info,
            process: Some(child),
        })
    }

    fn build_worker_prompt(
        worker_info: &WorkerInfo,
        system_prompt: &str,
        queue_name: &str,
    ) -> String {
        format!(
            r#"{system_prompt}

WORKER CONFIGURATION:
- Worker ID: {worker_id}
- Project: {project_id}
- Worker Type: {worker_type}
- Queue Name: {queue_name}

TASK PROCESSING INSTRUCTIONS:
1. You are a specialized worker for the vibe-ensemble multi-agent system
2. Your queue is: {queue_name}
3. Process tasks from your queue one by one
4. When queue is empty, exit gracefully
5. For each task, read the full ticket content including previous worker reports
6. Complete your stage and add a detailed report as a comment
7. Update the ticket's completed stage when done
8. Continue to next task or exit when queue is empty

Use the vibe-ensemble MCP server to:
- Get tasks from your queue: get_queue_tasks("{queue_name}")
- Get ticket details: get_ticket(ticket_id)
- Add your report: add_ticket_comment(ticket_id, worker_type, worker_id, stage_number, content)
- Update stage completion: complete_ticket_stage(ticket_id, stage)

Remember: You are working autonomously. Process tasks thoroughly and provide detailed reports for the next worker or coordinator.
"#,
            worker_id = worker_info.worker_id,
            project_id = worker_info.project_id,
            worker_type = worker_info.worker_type,
            queue_name = queue_name,
            system_prompt = system_prompt
        )
    }

    pub async fn stop_worker(
        state: &AppState,
        worker_id: &str,
    ) -> Result<bool> {
        info!("Stopping worker: {}", worker_id);

        // Check if worker exists and get PID
        let worker = Worker::get_by_id(&state.db, worker_id).await?;
        
        match worker {
            Some(worker) if worker.pid.is_some() => {
                let pid = worker.pid.unwrap();
                
                // Try to terminate process gracefully
                if let Ok(mut child) = tokio::process::Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .spawn() 
                {
                    let _ = child.wait().await;
                }

                // Wait a bit for graceful shutdown
                tokio::time::sleep(Duration::from_millis(1000)).await;

                // Force kill if still running
                let _ = tokio::process::Command::new("kill")
                    .arg("-KILL")
                    .arg(pid.to_string())
                    .spawn();

                // Update database status
                Worker::update_status(&state.db, worker_id, "finished", None).await?;

                // Create event
                crate::database::events::Event::create_worker_stopped(
                    &state.db,
                    worker_id,
                    "stopped by coordinator",
                ).await?;

                info!("Worker {} stopped", worker_id);
                Ok(true)
            }
            Some(_) => {
                warn!("Worker {} has no PID, marking as finished", worker_id);
                Worker::update_status(&state.db, worker_id, "finished", None).await?;
                Ok(true)
            }
            None => {
                warn!("Worker {} not found", worker_id);
                Ok(false)
            }
        }
    }

    pub async fn check_worker_health(state: &AppState, worker_id: &str) -> Result<WorkerStatus> {
        let worker = Worker::get_by_id(&state.db, worker_id).await?;
        
        match worker {
            Some(worker) => {
                if let Some(pid) = worker.pid {
                    // Check if process is still running
                    let is_running = tokio::process::Command::new("kill")
                        .arg("-0")
                        .arg(pid.to_string())
                        .status()
                        .await
                        .map(|status| status.success())
                        .unwrap_or(false);

                    if is_running {
                        // Update last activity
                        Worker::update_last_activity(&state.db, worker_id).await?;
                        Ok(WorkerStatus::Active)
                    } else {
                        // Process died, update status
                        Worker::update_status(&state.db, worker_id, "failed", None).await?;
                        
                        // Create event
                        crate::database::events::Event::create_worker_stopped(
                            &state.db,
                            worker_id,
                            "process died unexpectedly",
                        ).await?;

                        Ok(WorkerStatus::Failed)
                    }
                } else {
                    Ok(WorkerStatus::Spawning)
                }
            }
            None => Err(anyhow::anyhow!("Worker '{}' not found", worker_id)),
        }
    }
}
```

### 3. Queue Manager (`src/workers/queue.rs`)

```rust
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug};
use uuid::Uuid;

use super::types::{TaskItem, TaskQueue, QueueRegistry};

pub struct QueueManager {
    queues: QueueRegistry,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_queue(&self, queue_name: &str) -> Result<()> {
        info!("Creating queue: {}", queue_name);
        
        let mut queues = self.queues.write().await;
        if !queues.contains_key(queue_name) {
            queues.insert(queue_name.to_string(), RwLock::new(Vec::new()));
            info!("Queue '{}' created", queue_name);
        } else {
            debug!("Queue '{}' already exists", queue_name);
        }
        
        Ok(())
    }

    pub async fn delete_queue(&self, queue_name: &str) -> Result<bool> {
        info!("Deleting queue: {}", queue_name);
        
        let mut queues = self.queues.write().await;
        let removed = queues.remove(queue_name).is_some();
        
        if removed {
            info!("Queue '{}' deleted", queue_name);
        }
        
        Ok(removed)
    }

    pub async fn add_task(&self, queue_name: &str, ticket_id: &str) -> Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let task = TaskItem {
            task_id: task_id.clone(),
            ticket_id: ticket_id.to_string(),
            created_at: chrono::Utc::now(),
        };

        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let mut queue_items = queue.write().await;
            queue_items.push(task);
            info!("Task {} added to queue {}", task_id, queue_name);
            Ok(task_id)
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }

    pub async fn get_next_task(&self, queue_name: &str) -> Result<Option<TaskItem>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let mut queue_items = queue.write().await;
            if queue_items.is_empty() {
                Ok(None)
            } else {
                let task = queue_items.remove(0);
                debug!("Task {} retrieved from queue {}", task.task_id, queue_name);
                Ok(Some(task))
            }
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }

    pub async fn get_queue_status(&self, queue_name: &str) -> Result<Option<QueueStatus>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let queue_items = queue.read().await;
            Ok(Some(QueueStatus {
                queue_name: queue_name.to_string(),
                task_count: queue_items.len(),
                tasks: queue_items.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_queues(&self) -> Result<Vec<QueueStatus>> {
        let queues = self.queues.read().await;
        let mut result = Vec::new();
        
        for (queue_name, queue) in queues.iter() {
            let queue_items = queue.read().await;
            result.push(QueueStatus {
                queue_name: queue_name.clone(),
                task_count: queue_items.len(),
                tasks: queue_items.clone(),
            });
        }
        
        Ok(result)
    }

    pub async fn get_queue_tasks(&self, queue_name: &str) -> Result<Vec<TaskItem>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let queue_items = queue.read().await;
            Ok(queue_items.clone())
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueStatus {
    pub queue_name: String,
    pub task_count: usize,
    pub tasks: Vec<TaskItem>,
}
```

### 4. Worker Management Tools (`src/mcp/worker_tools.rs`)

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    database::workers::Worker,
    error::Result,
    server::AppState,
    workers::{
        process::ProcessManager,
        types::SpawnWorkerRequest,
    },
};
use super::tools::{
    ToolHandler, extract_param, create_success_response, create_error_response
};
use super::types::{CallToolResponse, Tool};

pub struct SpawnWorkerTool;

#[async_trait]
impl ToolHandler for SpawnWorkerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;

        let request = SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id,
            worker_type,
        };

        match ProcessManager::spawn_worker(state, request).await {
            Ok(worker_process) => {
                // Create queue for the worker
                let queue_name = &worker_process.info.queue_name;
                if let Err(e) = state.queue_manager.create_queue(queue_name).await {
                    return Ok(create_error_response(&format!("Worker spawned but failed to create queue: {}", e)));
                }

                let response = json!({
                    "worker_id": worker_process.info.worker_id,
                    "status": worker_process.info.status.as_str(),
                    "pid": worker_process.info.pid,
                    "queue_name": worker_process.info.queue_name
                });
                Ok(create_success_response(&format!("Worker spawned successfully: {}", response)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to spawn worker: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "spawn_worker".to_string(),
            description: "Spawn a new worker process".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Unique worker ID (format: worker_<type>_<number>)"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier"
                    }
                },
                "required": ["worker_id", "project_id", "worker_type"]
            }),
        }
    }
}

pub struct StopWorkerTool;

#[async_trait]
impl ToolHandler for StopWorkerTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;

        match ProcessManager::stop_worker(state, &worker_id).await {
            Ok(true) => Ok(create_success_response(&format!("Worker '{}' stopped successfully", worker_id))),
            Ok(false) => Ok(create_error_response(&format!("Worker '{}' not found", worker_id))),
            Err(e) => Ok(create_error_response(&format!("Failed to stop worker: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "stop_worker".to_string(),
            description: "Stop a running worker process".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID to stop"
                    }
                },
                "required": ["worker_id"]
            }),
        }
    }
}

pub struct ListWorkersTool;

#[async_trait]
impl ToolHandler for ListWorkersTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: Option<String> = super::tools::extract_optional_param(&arguments, "project_id")?;

        match Worker::list_by_project(&state.db, project_id.as_deref()).await {
            Ok(workers) => {
                let workers_json = serde_json::to_string_pretty(&workers)?;
                Ok(create_success_response(&format!("Workers:\n{}", workers_json)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to list workers: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_workers".to_string(),
            description: "List all workers, optionally filtered by project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to filter workers"
                    }
                }
            }),
        }
    }
}

pub struct GetWorkerStatusTool;

#[async_trait]
impl ToolHandler for GetWorkerStatusTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let worker_id: String = extract_param(&arguments, "worker_id")?;

        match ProcessManager::check_worker_health(state, &worker_id).await {
            Ok(status) => {
                let worker = Worker::get_by_id(&state.db, &worker_id).await?;
                match worker {
                    Some(worker) => {
                        let response = json!({
                            "worker_id": worker.worker_id,
                            "status": status.as_str(),
                            "pid": worker.pid,
                            "last_activity": worker.last_activity
                        });
                        Ok(create_success_response(&format!("Worker status: {}", response)))
                    }
                    None => Ok(create_error_response(&format!("Worker '{}' not found", worker_id))),
                }
            }
            Err(e) => Ok(create_error_response(&format!("Failed to get worker status: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_worker_status".to_string(),
            description: "Get the current status of a worker".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_id": {
                        "type": "string",
                        "description": "Worker ID to check"
                    }
                },
                "required": ["worker_id"]
            }),
        }
    }
}
```

### 5. Queue Management Tools (`src/mcp/queue_tools.rs`)

```rust
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{error::Result, server::AppState};
use super::tools::{
    ToolHandler, extract_param, create_success_response, create_error_response
};
use super::types::{CallToolResponse, Tool};

pub struct ListQueuesTool;

#[async_trait]
impl ToolHandler for ListQueuesTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        match state.queue_manager.list_queues().await {
            Ok(queues) => {
                let queues_json = serde_json::to_string_pretty(&queues)?;
                Ok(create_success_response(&format!("Queues:\n{}", queues_json)))
            }
            Err(e) => Ok(create_error_response(&format!("Failed to list queues: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_queues".to_string(),
            description: "List all task queues with their status".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

pub struct GetQueueStatusTool;

#[async_trait]
impl ToolHandler for GetQueueStatusTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        match state.queue_manager.get_queue_status(&queue_name).await {
            Ok(Some(status)) => {
                let status_json = serde_json::to_string_pretty(&status)?;
                Ok(create_success_response(&format!("Queue status:\n{}", status_json)))
            }
            Ok(None) => Ok(create_error_response(&format!("Queue '{}' not found", queue_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to get queue status: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_queue_status".to_string(),
            description: "Get the status of a specific queue".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue to check"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}

pub struct DeleteQueueTool;

#[async_trait]
impl ToolHandler for DeleteQueueTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let queue_name: String = extract_param(&arguments, "queue_name")?;

        match state.queue_manager.delete_queue(&queue_name).await {
            Ok(true) => Ok(create_success_response(&format!("Queue '{}' deleted successfully", queue_name))),
            Ok(false) => Ok(create_error_response(&format!("Queue '{}' not found", queue_name))),
            Err(e) => Ok(create_error_response(&format!("Failed to delete queue: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_queue".to_string(),
            description: "Delete a task queue".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "queue_name": {
                        "type": "string",
                        "description": "Name of the queue to delete"
                    }
                },
                "required": ["queue_name"]
            }),
        }
    }
}
```

### 6. Integration with App State

Update `src/server.rs`:
```rust
use crate::workers::queue::QueueManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub queue_manager: Arc<QueueManager>,
}

pub async fn run_server(config: Config) -> Result<()> {
    // Initialize database
    let db = crate::database::create_pool(&config.database_url()).await?;
    
    // Initialize queue manager
    let queue_manager = Arc::new(QueueManager::new());
    
    let state = AppState {
        config: config.clone(),
        db,
        queue_manager,
    };
    
    // ... rest of server setup
}
```

Update `src/mcp/server.rs` to register new tools:
```rust
use super::{worker_tools::*, queue_tools::*};

impl McpServer {
    pub fn new() -> Self {
        let mut tools = ToolRegistry::new();
        
        // ... existing project tools
        
        // Register worker management tools
        tools.register(SpawnWorkerTool);
        tools.register(StopWorkerTool);
        tools.register(ListWorkersTool);
        tools.register(GetWorkerStatusTool);
        
        // Register queue management tools
        tools.register(ListQueuesTool);
        tools.register(GetQueueStatusTool);
        tools.register(DeleteQueueTool);
        
        Self { tools }
    }
}
```

## Testing

### 1. Worker Spawning Test

```bash
# Create project and worker type first, then:
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "name": "spawn_worker",
      "arguments": {
        "worker_id": "worker_rust-dev_1",
        "project_id": "test/project",
        "worker_type": "rust-dev"
      }
    }
  }'
```

### 2. Queue Management Test

```bash
# List queues
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "call_tool",
    "params": {
      "name": "list_queues"
    }
  }'
```

## Validation Checklist

- [ ] Workers spawn successfully with Claude Code processes
- [ ] Worker status tracking works correctly
- [ ] Process health monitoring functions
- [ ] Queues are created automatically with workers
- [ ] Queue management tools work properly
- [ ] Worker-queue binding is maintained
- [ ] Database integration for workers is functional

## Next Steps

After completing Stage 4:
1. Test worker spawning and management thoroughly
2. Verify queue operations work correctly
3. Update progress in [TODO.md](../TODO.md)
4. Proceed to [Stage 5: Ticket System](STAGE_5_TICKET_SYSTEM.md)