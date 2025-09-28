use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::types::TaskItem;
use super::{claims::ClaimManager, process::ProcessManager};
use crate::{
    config::Config, database::DbPool, sse::EventBroadcaster,
    workers::domain::WorkerCompletionEvent, workers::transitions::TicketTransitionManager,
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
                    if let Err(release_error) =
                        ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
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
                    if let Err(release_error) =
                        ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
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
                if let Err(release_error) =
                    ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
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
                if let Err(release_error) =
                    ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
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

        // Emit event for worker processing start with both DB and SSE
        let emitter = crate::events::emitter::EventEmitter::new(&self.db, &self.event_broadcaster);
        if let Err(e) = emitter
            .emit_worker_started(&worker_id, &self.stage, &self.project_id)
            .await
        {
            warn!("Failed to emit worker_started event: {}", e);
        }

        match ProcessManager::spawn_worker(spawn_request).await {
            Ok(output) => {
                debug!(
                    worker_id = %worker_id,
                    ticket_id = %task.ticket_id,
                    "Worker completed successfully"
                );


                // Use the pipeline to determine the target stage
                let transition_manager = TicketTransitionManager::new(self.db.clone());
                let command = match output.outcome {
                    crate::workers::completion_processor::WorkerOutcome::NextStage => {
                        match transition_manager.get_next_stage(&task.ticket_id).await {
                            Ok(Some(next_stage)) => {
                                match crate::workers::domain::WorkerType::new(next_stage.clone()) {
                                    Ok(wt) => {
                                        crate::workers::domain::WorkerCommand::AdvanceToStage {
                                            target_stage: wt,
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to create WorkerType: {}", e);
                                        crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                            reason: format!("Failed to create WorkerType for stage '{}': {}", next_stage, e),
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                info!(
                                    "No next stage found for ticket {}, completing ticket",
                                    task.ticket_id
                                );
                                crate::workers::domain::WorkerCommand::CompleteTicket {
                                    resolution: "completed".to_string(),
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to get next stage for ticket {}: {}",
                                    task.ticket_id, e
                                );
                                crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                    reason: format!("Failed to get next stage for ticket: {}", e),
                                }
                            }
                        }
                    }
                    crate::workers::completion_processor::WorkerOutcome::PrevStage => {
                        match transition_manager.get_previous_stage(&task.ticket_id).await {
                            Ok(Some(prev_stage)) => {
                                match crate::workers::domain::WorkerType::new(prev_stage.clone()) {
                                    Ok(wt) => {
                                        crate::workers::domain::WorkerCommand::ReturnToStage {
                                            target_stage: wt,
                                            reason: output.reason.clone(),
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to create WorkerType: {}", e);
                                        crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                            reason: format!("Failed to create WorkerType for stage '{}': {}", prev_stage, e),
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                info!("No previous stage found for ticket {}, requesting coordinator attention", task.ticket_id);
                                crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                    reason: format!("Worker requested previous stage but ticket is at the beginning of pipeline: {}", output.reason),
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to get previous stage for ticket {}: {}",
                                    task.ticket_id, e
                                );
                                crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                    reason: format!(
                                        "Failed to get previous stage for ticket: {}",
                                        e
                                    ),
                                }
                            }
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
                    // Release the claim if we cannot forward the completion event
                    if let Err(release_error) =
                        ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
                    {
                        error!(
                            ticket_id = %task.ticket_id,
                            worker_id = %worker_id,
                            error = %release_error,
                            "Failed to release claim after completion send failure"
                        );
                    }
                } else {
                    // Emit event for worker completion only after successful send
                    let emitter =
                        crate::events::emitter::EventEmitter::new(&self.db, &self.event_broadcaster);
                    if let Err(e) = emitter
                        .emit_worker_completed(&worker_id, &self.stage, &self.project_id)
                        .await
                    {
                        warn!("Failed to emit worker_completed event: {}", e);
                    }
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
                if let Err(release_error) =
                    ClaimManager::release_ticket_claim_for_worker(&self.db, &task.ticket_id, &worker_id).await
                {
                    error!(
                        ticket_id = %task.ticket_id,
                        worker_id = %worker_id,
                        error = %release_error,
                        "Failed to release claim after worker failure"
                    );
                }

                // Emit event for worker failure with both DB and SSE
                let emitter =
                    crate::events::emitter::EventEmitter::new(&self.db, &self.event_broadcaster);
                let reason = format!("Worker process failed: {}", e);
                if let Err(emit_error) = emitter
                    .emit_worker_failed(
                        &worker_id,
                        &self.stage,
                        &self.project_id,
                        Some(reason.as_str()),
                    )
                    .await
                {
                    warn!("Failed to emit worker_failed event: {}", emit_error);
                }
            }
        }

        Ok(())
    }
}
