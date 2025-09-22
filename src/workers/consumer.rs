use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::types::TaskItem;
use super::{claims::ClaimManager, process::ProcessManager};
use crate::{
    config::Config,
    database::DbPool,
    events::EventPayload,
    sse::EventBroadcaster,
    workers::domain::WorkerCompletionEvent,
};

/// Manages individual consumer threads for project/stage combinations
pub struct WorkerConsumer {
    project_id: String,
    stage: String,
    config: Config,
    db: DbPool,
    completion_sender: mpsc::Sender<WorkerCompletionEvent>,
    event_broadcaster: EventBroadcaster,
}

impl WorkerConsumer {
    pub fn new(
        project_id: String,
        stage: String,
        config: Config,
        db: DbPool,
        completion_sender: mpsc::Sender<WorkerCompletionEvent>,
        event_broadcaster: EventBroadcaster,
    ) -> Self {
        Self {
            project_id,
            stage,
            config,
            db,
            completion_sender,
            event_broadcaster,
        }
    }

    /// Spawn and run the consumer loop for this project/stage combination
    pub async fn run(
        self: Arc<Self>,
        mut receiver: mpsc::Receiver<TaskItem>,
    ) -> Result<()> {
        let queue_key = format!("{}:{}", self.project_id, self.stage);
        info!(
            project_id = %self.project_id,
            stage = %self.stage,
            "Starting consumer for queue: {}"
        , queue_key);

        while let Some(task) = receiver.recv().await {
            if let Err(e) = self.process_task(task).await {
                error!(
                    project_id = %self.project_id,
                    stage = %self.stage,
                    error = %e,
                    "Failed to process task"
                );
            }
        }

        warn!(
            project_id = %self.project_id,
            stage = %self.stage,
            "Consumer loop ended"
        );
        Ok(())
    }

    /// Process a single task item
    async fn process_task(&self, task: TaskItem) -> Result<()> {
        debug!(
            project_id = %self.project_id,
            stage = %self.stage,
            ticket_id = %task.ticket_id,
            "Processing task"
        );

        // Attempt to claim the ticket for processing
        let ticket_id = crate::workers::domain::TicketId::new(task.ticket_id.clone()).map_err(|e| anyhow::anyhow!("Invalid ticket ID: {}", e))?;
        let worker_id = format!("{}:{}:{}", self.project_id, self.stage, ticket_id.as_str());
        let claim_result = ClaimManager::claim_for_processing(&self.db, &ticket_id, &worker_id).await;

        match claim_result {
            Ok(()) => {},
            Err(e) => {
                warn!(
                    ticket_id = %task.ticket_id,
                    project_id = %self.project_id,
                    stage = %self.stage,
                    error = %e,
                    "Failed to claim ticket for processing"
                );
                return Ok(()); // Not an error, ticket may be claimed by another process
            }
        }

        info!(
            ticket_id = %task.ticket_id,
            worker_id = %worker_id,
            project_id = %self.project_id,
            stage = %self.stage,
            "Claimed ticket for processing"
        );

        // Emit event for ticket processing start
        let event_payload = EventPayload::worker_started(&worker_id, &self.stage, &self.project_id);
        self.event_broadcaster.broadcast(event_payload);

        // Spawn the worker process
        let spawn_request = crate::workers::types::SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: self.project_id.clone(),
            worker_type: self.stage.clone(),
            queue_name: format!("{}:{}", self.project_id, self.stage),
            ticket_id: task.ticket_id.clone(),
            project_path: ".".to_string(),
            system_prompt: format!("Process ticket {} in stage {}", task.ticket_id, self.stage),
            server_host: self.config.host.clone(),
            server_port: self.config.port,
            permission_mode: self.config.permission_mode.clone(),
        };

        match ProcessManager::spawn_worker(spawn_request).await {
            Ok(output) => {
                debug!(
                    worker_id = %worker_id,
                    ticket_id = %task.ticket_id,
                    "Worker completed successfully"
                );

                // Send completion event for processing
                // Determine next stage from ticket pipeline or use current stage
                let target_stage = if let Ok(Some(ticket_info)) = crate::database::tickets::Ticket::get_by_id(&self.db, &task.ticket_id).await {
                    // Parse pipeline to find next stage
                    if let Ok(pipeline) = serde_json::from_str::<Vec<String>>(&ticket_info.ticket.execution_plan) {
                        if let Some(current_idx) = pipeline.iter().position(|s| s == &self.stage) {
                            if current_idx + 1 < pipeline.len() {
                                pipeline[current_idx + 1].clone()
                            } else {
                                "completed".to_string() // End of pipeline
                            }
                        } else {
                            warn!("Current stage '{}' not found in pipeline", self.stage);
                            "completed".to_string()
                        }
                    } else {
                        warn!("Failed to parse execution plan for ticket {}", task.ticket_id);
                        "completed".to_string()
                    }
                } else {
                    warn!("Failed to get ticket info for {}", task.ticket_id);
                    "completed".to_string()
                };

                let completion_event = WorkerCompletionEvent {
                    ticket_id: ticket_id.clone(),
                    command: crate::workers::domain::WorkerCommand::AdvanceToStage {
                        target_stage: match crate::workers::domain::WorkerType::new(target_stage) {
                            Ok(wt) => wt,
                            Err(e) => {
                                error!("Failed to create WorkerType: {}", e);
                                return Ok(()); // Skip this completion event
                            }
                        },
                        pipeline_update: None,
                    },
                    comment: output.message,
                };

                if let Err(e) = self.completion_sender.send(completion_event).await {
                    error!(
                        error = %e,
                        ticket_id = %task.ticket_id,
                        "Failed to send completion event"
                    );
                }
            }
            Err(e) => {
                error!(
                    worker_id = %worker_id,
                    ticket_id = %task.ticket_id,
                    error = %e,
                    "Worker process failed"
                );

                // Release the claim on failure
                if let Err(release_error) = ClaimManager::release_ticket_claim(&self.db, &self.event_broadcaster, &task.ticket_id).await {
                    error!(
                        ticket_id = %task.ticket_id,
                        worker_id = %worker_id,
                        error = %release_error,
                        "Failed to release claim after worker failure"
                    );
                }

                // Emit event for worker failure
                let event_payload = EventPayload::worker_failed(&worker_id, &self.stage, &self.project_id);
                self.event_broadcaster.broadcast(event_payload);
            }
        }

        Ok(())
    }
}