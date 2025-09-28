use crate::permissions::PermissionMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

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
    pub process: Option<tokio::process::Child>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnWorkerRequest {
    pub worker_id: String,
    pub project_id: String,
    pub worker_type: String,
    pub queue_name: String,
    pub ticket_id: String,
    pub project_path: String,
    pub system_prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_rules: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_patterns: Option<String>,
    pub server_host: String,
    pub server_port: u16,
    pub permission_mode: PermissionMode,
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
