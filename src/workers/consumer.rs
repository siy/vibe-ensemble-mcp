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

        // Note: Ticket is already claimed by QueueManager::submit_task() before being added to queue
        // We trust that the ticket is properly claimed and ready for processing

        // Create worker_id from raw ticket_id first (before validation) so guard can use it
        let worker_id = format!("{}:{}:{}", self.project_id, self.stage, &task.ticket_id);

        // Install cleanup guard BEFORE any fallible operations to avoid stuck claims
        let db_clone = self.db.clone();
        let ticket_id_clone = task.ticket_id.clone();
        let worker_id_clone = worker_id.clone();
        let claim_released = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let claim_released_guard = claim_released.clone();

        let _guard = scopeguard::guard((), move |_| {
            if !claim_released_guard.load(std::sync::atomic::Ordering::SeqCst) {
                warn!(
                    ticket_id = %ticket_id_clone,
                    worker_id = %worker_id_clone,
                    "Releasing claim due to error path (scopeguard cleanup)"
                );
                // Spawn async cleanup task instead of blocking the runtime
                tokio::spawn(async move {
                    if let Err(e) = ClaimManager::release_ticket_claim_for_worker(
                        &db_clone,
                        &ticket_id_clone,
                        &worker_id_clone,
                    )
                    .await
                    {
                        error!(
                            ticket_id = %ticket_id_clone,
                            worker_id = %worker_id_clone,
                            error = %e,
                            "CRITICAL: Failed to release claim in scopeguard cleanup"
                        );
                    }
                });
            }
        });

        // Now validate ticket_id after guard is installed
        let ticket_id = crate::workers::domain::TicketId::new(task.ticket_id.clone())
            .map_err(|e| anyhow::anyhow!("Invalid ticket ID: {}", e))?;

        info!(
            ticket_id = %task.ticket_id,
            worker_id = %worker_id,
            project_id = %self.project_id,
            stage = %self.stage,
            "Processing ticket (pre-claimed by queue manager)"
        );

        // Get ticket with project details (including rules and patterns)
        let ticket_with_project = match crate::database::tickets::Ticket::get_with_project_info(
            &self.db,
            &task.ticket_id,
        )
        .await
        {
            Ok(Some(ticket_info)) => ticket_info,
            Ok(None) => {
                error!(
                    ticket_id = %task.ticket_id,
                    "Ticket not found"
                );
                return Ok(()); // scopeguard will handle cleanup
            }
            Err(e) => {
                error!(
                    ticket_id = %task.ticket_id,
                    error = %e,
                    "Failed to fetch ticket with project details"
                );
                return Ok(()); // scopeguard will handle cleanup
            }
        };

        // Get project details from the ticket info
        let project =
            match crate::database::projects::Project::get_by_id(&self.db, &self.project_id).await {
                Ok(Some(project)) => project,
                Ok(None) => {
                    error!(
                        project_id = %self.project_id,
                        ticket_id = %task.ticket_id,
                        "Project not found"
                    );
                    return Ok(()); // scopeguard will handle cleanup
                }
                Err(e) => {
                    error!(
                        project_id = %self.project_id,
                        ticket_id = %task.ticket_id,
                        error = %e,
                        "Failed to fetch project details"
                    );
                    return Ok(()); // scopeguard will handle cleanup
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
                return Ok(()); // scopeguard will handle cleanup
            }
            Err(e) => {
                error!(
                    project_id = %self.project_id,
                    worker_type = %self.stage,
                    ticket_id = %task.ticket_id,
                    error = %e,
                    "Failed to fetch worker type details"
                );
                return Ok(()); // scopeguard will handle cleanup
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
            project_rules: ticket_with_project.project_rules,
            project_patterns: ticket_with_project.project_patterns,
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
                    crate::workers::completion_processor::WorkerOutcome::PlanningComplete => {
                        // Validate planning output
                        if output.tickets_to_create.is_empty() {
                            // Empty tickets with valid reason is acceptable (e.g., "no work needed")
                            if output.reason.to_lowercase().contains("no work")
                                || output.reason.to_lowercase().contains("no additional work")
                            {
                                info!(
                                    ticket_id = %task.ticket_id,
                                    "Planning complete with no work needed"
                                );
                                crate::workers::domain::WorkerCommand::CompleteTicket {
                                    resolution: "no_work_needed".to_string(),
                                }
                            } else {
                                warn!(
                                    ticket_id = %task.ticket_id,
                                    "Planning completed without tickets and without valid explanation"
                                );
                                crate::workers::domain::WorkerCommand::RequestCoordinatorAttention {
                                    reason: format!(
                                        "Planning completed but no tickets created and no explanation provided. Reason given: {}",
                                        output.reason
                                    ),
                                }
                            }
                        } else {
                            // Valid planning with tickets to create
                            info!(
                                ticket_id = %task.ticket_id,
                                ticket_count = output.tickets_to_create.len(),
                                worker_type_count = output.worker_types_needed.len(),
                                "Planning complete with {} tickets to create",
                                output.tickets_to_create.len()
                            );
                            crate::workers::domain::WorkerCommand::CompletePlanning {
                                tickets_to_create: output.tickets_to_create,
                                worker_types_needed: output.worker_types_needed,
                            }
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
                    return Ok(()); // scopeguard will handle cleanup
                }

                // Mark claim as released since completion event processor will handle it
                claim_released.store(true, std::sync::atomic::Ordering::SeqCst);

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
            Err(e) => {
                error!(
                    worker_id = %worker_id,
                    ticket_id = %task.ticket_id,
                    error = %e,
                    "Worker process failed"
                );

                // Determine if this is a validation failure or other error
                let error_msg = e.to_string();
                let is_validation_error = error_msg.contains("Invalid project path")
                    || error_msg.contains("does not exist")
                    || error_msg.contains("Invalid ticket ID")
                    || error_msg.contains("Invalid worker ID")
                    || error_msg.contains("Invalid system prompt");

                if is_validation_error {
                    // Place ticket on-hold with clear instructions for operator
                    info!(
                        ticket_id = %task.ticket_id,
                        "Placing ticket on-hold due to validation failure"
                    );

                    let on_hold_reason = format!(
                        "Worker spawn validation failed: {}. Please verify project configuration and use resume_ticket_processing() to retry.",
                        e
                    );

                    if let Err(hold_err) = crate::database::tickets::Ticket::place_on_hold(
                        &self.db,
                        &task.ticket_id,
                        &on_hold_reason,
                    )
                    .await
                    {
                        error!(
                            ticket_id = %task.ticket_id,
                            error = %hold_err,
                            "Failed to place ticket on-hold after validation failure"
                        );
                    }
                } else {
                    // For non-validation errors, just release claim (scopeguard handles this)
                    warn!(
                        ticket_id = %task.ticket_id,
                        "Worker spawn failed with non-validation error, claim will be released"
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
