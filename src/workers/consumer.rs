use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::types::TaskItem;
use super::{claims::ClaimManager, process::ProcessManager};
use crate::{
    config::Config, database::DbPool, events::EventPayload, sse::EventBroadcaster,
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
    pub async fn run(self: Arc<Self>, mut receiver: mpsc::Receiver<TaskItem>) -> Result<()> {
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
        let ticket_id = crate::workers::domain::TicketId::new(task.ticket_id.clone())
            .map_err(|e| anyhow::anyhow!("Invalid ticket ID: {}", e))?;
        let worker_id = format!("{}:{}:{}", self.project_id, self.stage, ticket_id.as_str());
        let claim_result =
            ClaimManager::claim_for_processing(&self.db, &ticket_id, &worker_id).await;

        match claim_result {
            Ok(()) => {}
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

        // Get project details to obtain the correct project path
        let project =
            match crate::database::projects::Project::get_by_id(&self.db, &self.project_id).await {
                Ok(Some(project)) => project,
                Ok(None) => {
                    error!(
                        project_id = %self.project_id,
                        ticket_id = %task.ticket_id,
                        "Project not found"
                    );
                    // Release the claim on project lookup failure
                    if let Err(release_error) = ClaimManager::release_ticket_claim(
                        &self.db,
                        &self.event_broadcaster,
                        &task.ticket_id,
                    )
                    .await
                    {
                        error!(
                            ticket_id = %task.ticket_id,
                            worker_id = %worker_id,
                            error = %release_error,
                            "Failed to release claim after project lookup failure"
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    error!(
                        project_id = %self.project_id,
                        ticket_id = %task.ticket_id,
                        error = %e,
                        "Failed to fetch project details"
                    );
                    // Release the claim on database error
                    if let Err(release_error) = ClaimManager::release_ticket_claim(
                        &self.db,
                        &self.event_broadcaster,
                        &task.ticket_id,
                    )
                    .await
                    {
                        error!(
                            ticket_id = %task.ticket_id,
                            worker_id = %worker_id,
                            error = %release_error,
                            "Failed to release claim after database error"
                        );
                    }
                    return Ok(());
                }
            };

        // Get the worker type details to get the proper system prompt
        let worker_type_data = match crate::database::worker_types::WorkerType::get_by_type(
            &self.db,
            &self.project_id,
            &self.stage,
        )
        .await
        {
            Ok(Some(wt)) => wt,
            Ok(None) => {
                error!(
                    project_id = %self.project_id,
                    worker_type = %self.stage,
                    ticket_id = %task.ticket_id,
                    "Worker type not found"
                );
                // Release the claim on worker type lookup failure
                if let Err(release_error) = ClaimManager::release_ticket_claim(
                    &self.db,
                    &self.event_broadcaster,
                    &task.ticket_id,
                )
                .await
                {
                    error!(
                        ticket_id = %task.ticket_id,
                        worker_id = %worker_id,
                        error = %release_error,
                        "Failed to release claim after worker type lookup failure"
                    );
                }
                return Ok(());
            }
            Err(e) => {
                error!(
                    project_id = %self.project_id,
                    worker_type = %self.stage,
                    ticket_id = %task.ticket_id,
                    error = %e,
                    "Failed to fetch worker type details"
                );
                // Release the claim on database error
                if let Err(release_error) = ClaimManager::release_ticket_claim(
                    &self.db,
                    &self.event_broadcaster,
                    &task.ticket_id,
                )
                .await
                {
                    error!(
                        ticket_id = %task.ticket_id,
                        worker_id = %worker_id,
                        error = %release_error,
                        "Failed to release claim after database error"
                    );
                }
                return Ok(());
            }
        };

        // Spawn the worker process
        let spawn_request = crate::workers::types::SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: self.project_id.clone(),
            worker_type: self.stage.clone(),
            queue_name: format!("{}:{}", self.project_id, self.stage),
            ticket_id: task.ticket_id.clone(),
            project_path: project.path,
            system_prompt: worker_type_data.system_prompt,
            server_host: self.config.host.clone(),
            server_port: self.config.port,
            permission_mode: self.config.permission_mode,
        };

        match ProcessManager::spawn_worker(spawn_request).await {
            Ok(output) => {
                debug!(
                    worker_id = %worker_id,
                    ticket_id = %task.ticket_id,
                    "Worker completed successfully"
                );

                // Use the worker's actual output to determine the command
                let command = match output.outcome {
                    crate::workers::completion_processor::WorkerOutcome::NextStage => {
                        if let Some(target_stage) = output.target_stage {
                            crate::workers::domain::WorkerCommand::AdvanceToStage {
                                target_stage: match crate::workers::domain::WorkerType::new(
                                    target_stage,
                                ) {
                                    Ok(wt) => wt,
                                    Err(e) => {
                                        error!("Failed to create WorkerType: {}", e);
                                        return Ok(()); // Skip this completion event
                                    }
                                },
                            }
                        } else {
                            error!("Worker specified next_stage but no target_stage provided");
                            return Ok(());
                        }
                    }
                    crate::workers::completion_processor::WorkerOutcome::PrevStage => {
                        // For now, treat as coordinator attention since we don't have ReturnToStage implemented
                        crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                            reason: "Worker requested previous stage".to_string(),
                        }
                    }
                    crate::workers::completion_processor::WorkerOutcome::CoordinatorAttention => {
                        crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                            reason: output.reason,
                        }
                    }
                };

                let completion_event = WorkerCompletionEvent {
                    ticket_id: ticket_id.clone(),
                    command,
                    comment: output.comment,
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
                if let Err(release_error) = ClaimManager::release_ticket_claim(
                    &self.db,
                    &self.event_broadcaster,
                    &task.ticket_id,
                )
                .await
                {
                    error!(
                        ticket_id = %task.ticket_id,
                        worker_id = %worker_id,
                        error = %release_error,
                        "Failed to release claim after worker failure"
                    );
                }

                // Emit event for worker failure
                let event_payload =
                    EventPayload::worker_failed(&worker_id, &self.stage, &self.project_id);
                self.event_broadcaster.broadcast(event_payload);
            }
        }

        Ok(())
    }
}
