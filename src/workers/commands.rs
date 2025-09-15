use super::domain::*;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct TaskSubmission {
    pub project_id: ProjectId,
    pub worker_type: WorkerType,
    pub ticket_id: TicketId,
}

impl TaskSubmission {
    pub fn new(
        project_id: String,
        worker_type: String,
        ticket_id: String,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            project_id: ProjectId::new(project_id)?,
            worker_type: WorkerType::new(worker_type)?,
            ticket_id: TicketId::new(ticket_id)?,
        })
    }

    pub fn queue_name(&self) -> QueueName {
        QueueName::new(&self.project_id, &self.worker_type)
    }
}

#[derive(Debug, Clone)]
pub struct WorkerRequest {
    pub submission: TaskSubmission,
    pub project_path: String,
    pub system_prompt: String,
    pub server_port: u16,
}

impl WorkerRequest {
    pub fn new(
        submission: TaskSubmission,
        project_path: String,
        system_prompt: String,
        server_port: u16,
    ) -> Self {
        Self {
            submission,
            project_path,
            system_prompt,
            server_port,
        }
    }
}