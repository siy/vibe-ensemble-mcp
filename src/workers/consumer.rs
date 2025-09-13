use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};
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

    /// Spawn a consumer for a specific stage without Send issues
    pub fn spawn_consumer_for_stage(
        project_id: String,
        worker_type: String,
        state: Arc<AppState>,
    ) -> Result<()> {
        let consumer = WorkerConsumer::new(project_id.clone(), worker_type.clone(), state);

        tokio::spawn(async move {
            if let Err(e) = consumer.start().await {
                error!(
                    "Consumer thread for {}-{} failed: {}",
                    project_id, worker_type, e
                );
            }
        });

        Ok(())
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
            trace!(
                "[WorkerConsumer] Main loop iteration for {}-{}",
                self.project_id,
                self.worker_type
            );

            // Check for shutdown signal
            if *self.shutdown.read().await {
                info!(
                    "WorkerConsumer {}-{} shutting down",
                    self.project_id, self.worker_type
                );
                trace!("[WorkerConsumer] Shutdown signal received, breaking from main loop");
                break;
            }
            trace!("[WorkerConsumer] No shutdown signal, proceeding with ticket processing");

            // Try to process one ticket
            trace!(
                "[WorkerConsumer] Calling process_next_ticket for {}-{}",
                self.project_id,
                self.worker_type
            );
            match self.process_next_ticket().await {
                Ok(processed) => {
                    trace!(
                        "[WorkerConsumer] process_next_ticket returned: processed={}",
                        processed
                    );
                    if !processed {
                        // No tickets available, sleep briefly
                        trace!("[WorkerConsumer] No tickets processed, sleeping for 500ms");
                        sleep(Duration::from_millis(500)).await;
                        trace!("[WorkerConsumer] Woke up from 500ms sleep");
                    }
                }
                Err(e) => {
                    error!(
                        "Error processing ticket in consumer {}-{}: {}",
                        self.project_id, self.worker_type, e
                    );
                    trace!("[WorkerConsumer] Error in ticket processing, sleeping for 5s to prevent tight loops");
                    // Sleep on error to prevent tight loops
                    sleep(Duration::from_secs(5)).await;
                    trace!("[WorkerConsumer] Woke up from 5s error sleep");
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
        trace!(
            "[WorkerConsumer] process_next_ticket started for {}-{}",
            self.project_id,
            self.worker_type
        );

        // Get next ticket from queue
        trace!("[WorkerConsumer] Attempting to get next task from worker queue");
        let task_item = match self
            .state
            .queue_manager
            .get_next_task_from_worker_queue(&self.project_id, &self.worker_type)
            .await?
        {
            Some(task) => {
                trace!("[WorkerConsumer] Got task from queue: {:?}", task);
                task
            }
            None => {
                trace!(
                    "[WorkerConsumer] No tasks in queue, checking database for unclaimed tickets"
                );
                // Also check database for tickets in this stage that aren't claimed
                return self.check_database_for_tickets().await;
            }
        };

        debug!(
            "Processing ticket {} from queue {}",
            task_item.ticket_id, self.queue_name
        );
        trace!(
            "[WorkerConsumer] About to process ticket: {}",
            task_item.ticket_id
        );

        // Process the ticket
        let result = self.process_ticket(&task_item.ticket_id).await;
        trace!(
            "[WorkerConsumer] process_ticket completed with result: {:?}",
            result
        );
        result
    }

    /// Check database for unclaimed tickets in the current stage
    async fn check_database_for_tickets(&self) -> Result<bool> {
        trace!(
            "[WorkerConsumer] check_database_for_tickets started for {}-{}",
            self.project_id,
            self.worker_type
        );

        // Find tickets for this project that are in the current stage and not claimed
        trace!("[WorkerConsumer] Querying database for unclaimed tickets");
        let tickets =
            Ticket::get_by_stage_unclaimed(&self.state.db, &self.project_id, &self.worker_type)
                .await?;

        trace!(
            "[WorkerConsumer] Database query returned {} unclaimed tickets",
            tickets.len()
        );

        if tickets.is_empty() {
            trace!("[WorkerConsumer] No unclaimed tickets found in database");
            return Ok(false);
        }

        // Process the first unclaimed ticket
        let ticket = &tickets[0];
        debug!(
            "Found unclaimed ticket {} in stage {} for project {}",
            ticket.ticket_id, ticket.current_stage, self.project_id
        );
        trace!(
            "[WorkerConsumer] Selected ticket for processing: {:?}",
            ticket
        );

        let result = self.process_ticket(&ticket.ticket_id).await;
        trace!(
            "[WorkerConsumer] Database ticket processing result: {:?}",
            result
        );
        result
    }

    /// Process a specific ticket through its lifecycle
    async fn process_ticket(&self, ticket_id: &str) -> Result<bool> {
        trace!(
            "[WorkerConsumer] process_ticket started for ticket_id: {}",
            ticket_id
        );

        // Claim the ticket
        trace!("[WorkerConsumer] Attempting to claim ticket: {}", ticket_id);
        if !self.claim_ticket(ticket_id).await? {
            debug!("Ticket {} already claimed by another worker", ticket_id);
            trace!(
                "[WorkerConsumer] Ticket {} already claimed, returning true (processed)",
                ticket_id
            );
            return Ok(true); // Still processed something
        }
        trace!(
            "[WorkerConsumer] Successfully claimed ticket: {}",
            ticket_id
        );

        info!(
            "Processing ticket {} with worker type {}",
            ticket_id, self.worker_type
        );

        // Spawn worker in dedicated thread to avoid Send constraints
        trace!(
            "[WorkerConsumer] About to spawn worker for ticket: {}",
            ticket_id
        );
        let worker_result = self.spawn_and_wait_for_worker(ticket_id).await;
        trace!(
            "[WorkerConsumer] Worker spawn completed with result: {:?}",
            worker_result.as_ref().map(|_| "Ok").unwrap_or("Err")
        );

        match worker_result {
            Ok(output) => {
                trace!(
                    "[WorkerConsumer] Worker succeeded, processing output: {:?}",
                    output
                );
                // Process worker output and handle stage transition
                self.process_worker_output(ticket_id, &output).await?;
                trace!(
                    "[WorkerConsumer] Worker output processed successfully for ticket: {}",
                    ticket_id
                );
                Ok(true)
            }
            Err(e) => {
                error!("Worker failed for ticket {}: {}", ticket_id, e);
                trace!(
                    "[WorkerConsumer] Worker failed, handling failure for ticket: {}",
                    ticket_id
                );
                // Put ticket on hold and create coordinator event
                self.handle_worker_failure(ticket_id, &format!("Worker failed: {}", e))
                    .await?;
                trace!(
                    "[WorkerConsumer] Worker failure handled for ticket: {}",
                    ticket_id
                );
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
        trace!(
            "[WorkerConsumer] claim_ticket: ticket_id={}, generated_worker_id={}",
            ticket_id,
            worker_id
        );

        // Try to claim the ticket atomically
        trace!("[WorkerConsumer] Attempting atomic ticket claim");
        let updated = Ticket::claim_for_processing(&self.state.db, ticket_id, &worker_id).await?;
        trace!(
            "[WorkerConsumer] Claim attempt result: {} rows updated",
            updated
        );

        if updated > 0 {
            debug!(
                "Successfully claimed ticket {} with worker {}",
                ticket_id, worker_id
            );
            trace!("[WorkerConsumer] Ticket claim successful");
            Ok(true)
        } else {
            trace!("[WorkerConsumer] Ticket claim failed - already claimed by another worker");
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
        trace!(
            "[WorkerConsumer] spawn_and_wait_for_worker: ticket_id={}, worker_id={}",
            ticket_id,
            worker_id
        );

        // Create worker request - this will be simplified to not use log files
        let spawn_request = crate::workers::types::SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: self.project_id.clone(),
            worker_type: self.worker_type.clone(),
            queue_name: self.queue_name.clone(),
        };
        trace!(
            "[WorkerConsumer] Created spawn request: {:?}",
            spawn_request
        );

        // Spawn worker in a blocking thread to avoid Send constraints
        trace!("[WorkerConsumer] Spawning worker in blocking thread");
        let state = self.state.clone();
        let ticket_id = ticket_id.to_string();

        let output = tokio::task::spawn_blocking(move || {
            trace!("[WorkerConsumer] Inside blocking thread");
            // This will be implemented to use stdout JSON instead of log parsing
            tokio::runtime::Handle::current().block_on(async {
                trace!("[WorkerConsumer] Calling spawn_worker_simplified");
                // For now, simulate the simplified worker spawning
                Self::spawn_worker_simplified(&state, &spawn_request, &ticket_id).await
            })
        })
        .await??;

        trace!(
            "[WorkerConsumer] Worker spawning completed, output: {:?}",
            output
        );
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
        trace!(
            "[WorkerConsumer] process_worker_output: ticket_id={}, output={:?}",
            ticket_id,
            output
        );

        match output.outcome.as_str() {
            "next_stage" => {
                trace!("[WorkerConsumer] Processing next_stage outcome");
                if let Some(target_stage) = &output.target_stage {
                    trace!("[WorkerConsumer] Target stage provided: {}", target_stage);

                    // Update ticket to next stage
                    trace!(
                        "[WorkerConsumer] Updating ticket stage to: {}",
                        target_stage
                    );
                    Ticket::update_stage(&self.state.db, ticket_id, target_stage).await?;
                    trace!("[WorkerConsumer] Ticket stage updated successfully");

                    // Add comment about the transition
                    trace!("[WorkerConsumer] Adding transition comment");
                    crate::database::comments::Comment::create(
                        &self.state.db,
                        ticket_id,
                        Some(&self.worker_type),
                        None,
                        None,
                        &format!("Stage completed: {}", output.comment),
                    )
                    .await?;
                    trace!("[WorkerConsumer] Transition comment added");

                    // Add ticket to the next stage's queue
                    trace!(
                        "[WorkerConsumer] Enqueueing ticket for next stage: {}",
                        target_stage
                    );
                    self.enqueue_for_next_stage(ticket_id, target_stage).await?;
                    trace!("[WorkerConsumer] Ticket enqueued for next stage successfully");

                    info!(
                        "Ticket {} transitioned to stage {}",
                        ticket_id, target_stage
                    );
                } else {
                    warn!("Worker returned next_stage but no target_stage provided");
                    trace!("[WorkerConsumer] next_stage outcome missing target_stage field");
                }
            }
            "prev_stage" => {
                trace!("[WorkerConsumer] Processing prev_stage outcome");
                // Handle returning to previous stage
                warn!(
                    "Worker requested return to previous stage for ticket {}: {}",
                    ticket_id, output.reason
                );
                trace!("[WorkerConsumer] prev_stage logic not yet implemented");
                // TODO: Implement previous stage logic
            }
            "coordinator_attention" => {
                trace!("[WorkerConsumer] Processing coordinator_attention outcome");
                // Put ticket on hold and create coordinator event
                self.handle_coordinator_attention(ticket_id, &output.reason)
                    .await?;
                trace!("[WorkerConsumer] Coordinator attention handled");
            }
            _ => {
                warn!("Unknown worker outcome: {}", output.outcome);
                trace!(
                    "[WorkerConsumer] Unknown outcome received: {}",
                    output.outcome
                );
            }
        }

        trace!("[WorkerConsumer] process_worker_output completed successfully");
        Ok(())
    }

    /// Add ticket to the next stage's queue and ensure consumer thread exists
    async fn enqueue_for_next_stage(&self, ticket_id: &str, target_stage: &str) -> Result<()> {
        trace!(
            "[WorkerConsumer] enqueue_for_next_stage: ticket_id={}, target_stage={}, project_id={}",
            ticket_id,
            target_stage,
            self.project_id
        );

        // Create the queue for the target stage if it doesn't exist
        trace!(
            "[WorkerConsumer] Ensuring queue exists for target stage: {}",
            target_stage
        );
        self.state
            .queue_manager
            .create_queue(&self.project_id, target_stage)
            .await?;

        // Add ticket to the appropriate queue for the target stage
        let task_id = self
            .state
            .queue_manager
            .add_task_to_worker_queue(&self.project_id, target_stage, ticket_id)
            .await?;

        debug!(
            "Added ticket {} to queue for stage {}",
            ticket_id, target_stage
        );
        trace!("[WorkerConsumer] Ticket enqueued with task_id: {}", task_id);

        // Start consumer thread for the target stage if not already running
        trace!(
            "[WorkerConsumer] Starting consumer thread for target stage: {}",
            target_stage
        );
        Self::spawn_consumer_for_stage(
            self.project_id.clone(),
            target_stage.to_string(),
            self.state.clone(),
        )?;

        info!(
            "Started consumer thread for project={}, worker_type={}",
            self.project_id, target_stage
        );
        trace!("[WorkerConsumer] Consumer thread spawned for target stage successfully");
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
