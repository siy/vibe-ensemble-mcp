use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueueName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerType(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TicketId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl QueueName {
    pub fn new(project_id: &ProjectId, worker_type: &WorkerType) -> Self {
        Self(format!("{}-{}-queue", project_id.0, worker_type.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ProjectId {
    pub fn new(id: String) -> Result<Self, DomainError> {
        if id.trim().is_empty() {
            return Err(DomainError::InvalidProjectId);
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WorkerType {
    pub fn new(type_name: String) -> Result<Self, DomainError> {
        if type_name.trim().is_empty() {
            return Err(DomainError::InvalidWorkerType);
        }
        Ok(Self(type_name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TicketId {
    pub fn new(id: String) -> Result<Self, DomainError> {
        if id.trim().is_empty() {
            return Err(DomainError::InvalidTicketId);
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract ticket ID from worker ID format: "project_id:stage:ticket_id"
    pub fn extract_from_worker_id(worker_id: &str) -> Option<String> {
        worker_id.split(':').nth(2).map(|s| s.to_string())
    }
}

impl TaskId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QueueName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for WorkerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerCommand {
    AdvanceToStage {
        target_stage: WorkerType,
        pipeline_update: Option<Vec<WorkerType>>,
    },
    ReturnToStage {
        target_stage: WorkerType,
        reason: String,
    },
    RequestCoordinatorAttention {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCompletionEvent {
    pub ticket_id: TicketId,
    pub command: WorkerCommand,
    pub comment: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid project ID: cannot be empty")]
    InvalidProjectId,
    #[error("Invalid worker type: cannot be empty")]
    InvalidWorkerType,
    #[error("Invalid ticket ID: cannot be empty")]
    InvalidTicketId,
}
