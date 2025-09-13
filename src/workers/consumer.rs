use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::queue::QueueManager;
use crate::database::tickets::Ticket;
use crate::server::AppState;

/// Consumer thread that processes tickets for a specific project-worker type combination
pub struct WorkerConsumer {
    project_id: String,
    worker_type: String,
    queue_name: String,
    state: Arc<AppState>,
    shutdown: Arc<RwLock<bool>>,
}

impl WorkerConsumer {
    pub fn new(project_id: String, worker_type: String, state: Arc<AppState>) -> Self {
        let queue_name = QueueManager::generate_queue_name(&project_id, &worker_type);

        Self {
            project_id,
            worker_type,
            queue_name,
            state,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the consumer thread - runs continuously until shutdown
    pub async fn start(self) -> Result<()> {
        info!(
            "Starting WorkerConsumer for {}-{}",
            self.project_id, self.worker_type
        );

        // Create the queue if it doesn't exist
        self.state
            .queue_manager
            .create_queue(&self.project_id, &self.worker_type)
            .await?;

        // Main processing loop
        loop {
            // Check for shutdown signal
            if *self.shutdown.read().await {
                info!(
                    "WorkerConsumer {}-{} shutting down",
                    self.project_id, self.worker_type
                );
                break;
            }

            // Try to process one ticket
            match self.process_next_ticket().await {
                Ok(processed) => {
                    if !processed {
                        // No tickets available, sleep briefly
                        sleep(Duration::from_millis(500)).await;
                    }
                }
                Err(e) => {
                    error!(
                        "Error processing ticket in consumer {}-{}: {}",
                        self.project_id, self.worker_type, e
                    );
                    // Sleep on error to prevent tight loops
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        Ok(())
    }

    /// Signal the consumer to shutdown gracefully
    pub async fn shutdown(&self) {
        *self.shutdown.write().await = true;
    }

    /// Process one ticket from the queue
    async fn process_next_ticket(&self) -> Result<bool> {
        // Get next ticket from queue
        let task_item = match self
            .state
            .queue_manager
            .get_next_task_from_worker_queue(&self.project_id, &self.worker_type)
            .await?
        {
            Some(task) => task,
            None => {
                // Also check database for tickets in this stage that aren't claimed
                return self.check_database_for_tickets().await;
            }
        };

        debug!(
            "Processing ticket {} from queue {}",
            task_item.ticket_id, self.queue_name
        );

        // Process the ticket
        self.process_ticket(&task_item.ticket_id).await
    }

    /// Check database for unclaimed tickets in the current stage
    async fn check_database_for_tickets(&self) -> Result<bool> {
        // Find tickets for this project that are in the current stage and not claimed
        let tickets =
            Ticket::get_by_stage_unclaimed(&self.state.db, &self.project_id, &self.worker_type)
                .await?;

        if tickets.is_empty() {
            return Ok(false);
        }

        // Process the first unclaimed ticket
        let ticket = &tickets[0];
        debug!(
            "Found unclaimed ticket {} in stage {} for project {}",
            ticket.ticket_id, ticket.current_stage, self.project_id
        );

        self.process_ticket(&ticket.ticket_id).await
    }

    /// Process a specific ticket through its lifecycle
    async fn process_ticket(&self, ticket_id: &str) -> Result<bool> {
        // Claim the ticket
        if !self.claim_ticket(ticket_id).await? {
            debug!("Ticket {} already claimed by another worker", ticket_id);
            return Ok(true); // Still processed something
        }

        info!(
            "Processing ticket {} with worker type {}",
            ticket_id, self.worker_type
        );

        // Spawn worker in dedicated thread to avoid Send constraints
        let worker_result = self.spawn_and_wait_for_worker(ticket_id).await;

        match worker_result {
            Ok(output) => {
                // Process worker output and handle stage transition
                self.process_worker_output(ticket_id, &output).await?;
                Ok(true)
            }
            Err(e) => {
                error!("Worker failed for ticket {}: {}", ticket_id, e);
                // Put ticket on hold and create coordinator event
                self.handle_worker_failure(ticket_id, &format!("Worker failed: {}", e))
                    .await?;
                Ok(true)
            }
        }
    }

    /// Claim a ticket for processing
    async fn claim_ticket(&self, ticket_id: &str) -> Result<bool> {
        let worker_id = format!(
            "consumer-{}-{}",
            self.worker_type,
            &Uuid::new_v4().to_string()[..8]
        );

        // Try to claim the ticket atomically
        let updated = Ticket::claim_for_processing(&self.state.db, ticket_id, &worker_id).await?;

        if updated > 0 {
            debug!(
                "Successfully claimed ticket {} with worker {}",
                ticket_id, worker_id
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Spawn worker and wait for completion
    async fn spawn_and_wait_for_worker(&self, ticket_id: &str) -> Result<WorkerOutput> {
        let worker_id = format!(
            "worker-{}-{}",
            self.worker_type,
            &Uuid::new_v4().to_string()[..8]
        );

        // Create worker request - this will be simplified to not use log files
        let spawn_request = crate::workers::types::SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: self.project_id.clone(),
            worker_type: self.worker_type.clone(),
            queue_name: self.queue_name.clone(),
        };

        // Spawn worker in a blocking thread to avoid Send constraints
        let state = self.state.clone();
        let ticket_id = ticket_id.to_string();

        let output = tokio::task::spawn_blocking(move || {
            // This will be implemented to use stdout JSON instead of log parsing
            tokio::runtime::Handle::current().block_on(async {
                // For now, simulate the simplified worker spawning
                Self::spawn_worker_simplified(&state, &spawn_request, &ticket_id).await
            })
        })
        .await??;

        Ok(output)
    }

    /// Simplified worker spawning that outputs single JSON response
    async fn spawn_worker_simplified(
        _state: &AppState,
        request: &crate::workers::types::SpawnWorkerRequest,
        ticket_id: &str,
    ) -> Result<WorkerOutput> {
        // For now, return a placeholder until we implement the full worker spawning
        info!(
            "Worker spawning for ticket {} - using placeholder",
            ticket_id
        );
        Ok(WorkerOutput {
            outcome: "next_stage".to_string(),
            target_stage: Some("completed".to_string()),
            comment: format!(
                "Processed ticket {} with worker type {}",
                ticket_id, request.worker_type
            ),
            reason: "Placeholder implementation".to_string(),
        })
    }

    /// Process worker output and handle stage transitions
    async fn process_worker_output(&self, ticket_id: &str, output: &WorkerOutput) -> Result<()> {
        info!(
            "Processing worker output for ticket {}: outcome={}",
            ticket_id, output.outcome
        );

        match output.outcome.as_str() {
            "next_stage" => {
                if let Some(target_stage) = &output.target_stage {
                    // Update ticket to next stage
                    Ticket::update_stage(&self.state.db, ticket_id, target_stage).await?;

                    // Add comment about the transition
                    crate::database::comments::Comment::create(
                        &self.state.db,
                        ticket_id,
                        Some(&self.worker_type),
                        None,
                        None,
                        &format!("Stage completed: {}", output.comment),
                    )
                    .await?;

                    // Add ticket to the next stage's queue
                    self.enqueue_for_next_stage(ticket_id, target_stage).await?;

                    info!(
                        "Ticket {} transitioned to stage {}",
                        ticket_id, target_stage
                    );
                } else {
                    warn!("Worker returned next_stage but no target_stage provided");
                }
            }
            "prev_stage" => {
                // Handle returning to previous stage
                warn!(
                    "Worker requested return to previous stage for ticket {}: {}",
                    ticket_id, output.reason
                );
                // TODO: Implement previous stage logic
            }
            "coordinator_attention" => {
                // Put ticket on hold and create coordinator event
                self.handle_coordinator_attention(ticket_id, &output.reason)
                    .await?;
            }
            _ => {
                warn!("Unknown worker outcome: {}", output.outcome);
            }
        }

        Ok(())
    }

    /// Add ticket to the next stage's queue
    async fn enqueue_for_next_stage(&self, ticket_id: &str, target_stage: &str) -> Result<()> {
        // Add ticket to the appropriate queue for the target stage
        self.state
            .queue_manager
            .add_task_to_worker_queue(&self.project_id, target_stage, ticket_id)
            .await?;

        debug!(
            "Added ticket {} to queue for stage {}",
            ticket_id, target_stage
        );
        Ok(())
    }

    /// Handle worker failure
    async fn handle_worker_failure(&self, ticket_id: &str, error_msg: &str) -> Result<()> {
        // Put ticket on hold
        Ticket::update_state(&self.state.db, ticket_id, "on_hold").await?;

        // Create coordinator event with appropriate type based on error
        let event_type = if error_msg.contains("crash") || error_msg.contains("panic") {
            "worker_crash"
        } else if error_msg.contains("timeout") || error_msg.contains("killed") {
            "worker_failure"
        } else {
            "worker_processing_outcome"
        };

        crate::database::events::Event::create(
            &self.state.db,
            event_type,
            Some(ticket_id),
            None,
            Some(&self.worker_type),
            Some(error_msg),
        )
        .await?;

        warn!(
            "Ticket {} put on hold due to {}: {}",
            ticket_id, event_type, error_msg
        );
        Ok(())
    }

    /// Handle coordinator attention request
    async fn handle_coordinator_attention(&self, ticket_id: &str, reason: &str) -> Result<()> {
        // Put ticket on hold
        Ticket::update_state(&self.state.db, ticket_id, "on_hold").await?;

        // Create coordinator event with proper type for attention requests
        crate::database::events::Event::create(
            &self.state.db,
            "coordinator_attention",
            Some(ticket_id),
            None,
            Some(&self.worker_type),
            Some(reason),
        )
        .await?;

        info!(
            "Ticket {} requires coordinator attention: {}",
            ticket_id, reason
        );
        Ok(())
    }
}

/// Worker output structure for JSON parsing
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkerOutput {
    pub outcome: String,
    pub target_stage: Option<String>,
    pub comment: String,
    pub reason: String,
}
