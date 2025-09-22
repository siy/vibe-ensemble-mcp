use anyhow::Result;
use dashmap::DashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{
    claims::ClaimManager,
    consumer::WorkerConsumer,
    dependencies::DependencyManager,
    pipeline::PipelineManager,
    types::TaskItem,
};
use crate::{
    config::Config,
    database::{tickets::{DependencyStatus, TicketState}, DbPool},
    events::EventPayload,
    sse::EventBroadcaster,
    workers::domain::{TicketId, WorkerCommand, WorkerCompletionEvent, WorkerType},
};
use tracing::trace;
use uuid::Uuid;

// Simplified worker output for this implementation
#[derive(Debug, Clone)]
pub struct WorkerOutput {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum WorkerOutcome {
    NextStage,
    PrevStage,
    CoordinatorAttention,
}

/// Default buffer size for bounded channels
const DEFAULT_CHANNEL_BUFFER_SIZE: usize = 1000;

pub struct QueueManager {
    queues: DashMap<String, mpsc::Sender<TaskItem>>,
    completion_sender: mpsc::Sender<WorkerCompletionEvent>,
    config: Config,
    event_broadcaster: EventBroadcaster,
    db: DbPool,
}

// QueueManager intentionally does not implement Default to prevent misuse
// Always use QueueManager::new(db, config, event_broadcaster) for proper initialization

impl fmt::Debug for QueueManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueManager")
            .field("queue_count", &self.queues.len())
            .finish()
    }
}

impl QueueManager {
    pub fn new(db: DbPool, config: Config, event_broadcaster: EventBroadcaster) -> Arc<Self> {
        let (completion_sender, completion_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER_SIZE);

        let queue_manager = Arc::new(Self {
            queues: DashMap::new(),
            completion_sender,
            config,
            event_broadcaster,
            db,
        });

        // Spawn the completion event processor thread internally
        let queue_manager_clone = queue_manager.clone();
        tokio::spawn(async move {
            queue_manager_clone
                .start_completion_event_processor(completion_receiver)
                .await;
        });

        queue_manager
    }

    /// Get a sender for WorkerCompletionEvent processing
    pub fn get_completion_sender(&self) -> mpsc::Sender<WorkerCompletionEvent> {
        self.completion_sender.clone()
    }

    /// Generate standardized queue name: "{project_id}-{worker_type}-queue"
    pub fn generate_queue_name(project_id: &str, worker_type: &str) -> String {
        format!("{}-{}-queue", project_id, worker_type)
    }

    /// Submit task to worker queue - creates queue and spawns consumer if needed
    /// Claims the ticket before submission
    pub async fn submit_task(
        self: &Arc<Self>,
        project_id: &str,
        worker_type: &str,
        ticket_id: &str,
    ) -> Result<String> {
        let queue_name = Self::generate_queue_name(project_id, worker_type);
        let task_id = Uuid::new_v4().to_string();

        trace!(
            "[QueueManager] submit_task: project_id={}, worker_type={}, ticket_id={}, task_id={}",
            project_id,
            worker_type,
            ticket_id,
            task_id
        );

        // Validate that the worker type exists for this project
        let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
            &self.db,
            project_id,
            worker_type,
        )
        .await?;

        if worker_type_exists.is_none() {
            return Err(anyhow::anyhow!(
                "Worker type '{}' does not exist for project '{}'. Cannot submit task for ticket {}",
                worker_type,
                project_id,
                ticket_id
            ));
        }

        // Ensure ticket is open and ready (dependency_status)
        let readiness = sqlx::query_as::<_, (String, String)>(
            "SELECT state, dependency_status FROM tickets WHERE ticket_id = ?1",
        )
        .bind(ticket_id)
        .fetch_optional(&self.db)
        .await?;
        if let Some((state, dep)) = readiness {
            let state_enum: Result<TicketState, _> = state.parse();
            let dep_enum: Result<DependencyStatus, _> = dep.parse();

            if !matches!(state_enum.ok(), Some(TicketState::Open))
                || !matches!(dep_enum.ok(), Some(DependencyStatus::Ready)) {
                return Err(anyhow::anyhow!(
                    "Ticket {} is not ready (state='{}', dependency_status='{}')",
                    ticket_id,
                    state,
                    dep
                ));
            }
        } else {
            return Err(anyhow::anyhow!(format!("Ticket '{}' not found", ticket_id)));
        }

        // Claim the ticket before submitting to queue
        let worker_id = format!("consumer-{}-{}", worker_type, &task_id[..8]);
        let ticket_id_domain = TicketId::new(ticket_id.to_string())?;
        ClaimManager::claim_for_processing(&self.db, &ticket_id_domain, &worker_id).await?;

        info!(
            "[QueueManager] Claimed ticket {} with worker {}",
            ticket_id, worker_id
        );

        // Notify about ticket being claimed for processing
        let event = EventPayload::ticket_updated(ticket_id, project_id, "claimed");
        self.event_broadcaster.broadcast(event);

        let task = TaskItem {
            task_id: task_id.clone(),
            ticket_id: ticket_id.to_string(),
            created_at: chrono::Utc::now(),
        };

        // Get or create queue with consumer
        let sender = match self
            .get_or_create_queue(&queue_name, project_id, worker_type)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let _ = ClaimManager::release_ticket_if_claimed(
                    &self.db,
                    &self.event_broadcaster,
                    &ticket_id_domain,
                )
                .await;
                return Err(e);
            }
        };

        // Send task to queue
        if sender.send(task).await.is_err() {
            let _ = ClaimManager::release_ticket_if_claimed(
                &self.db,
                &self.event_broadcaster,
                &ticket_id_domain,
            )
            .await;
            return Err(anyhow::anyhow!("Queue {} is closed", queue_name));
        }

        debug!(
            "[QueueManager] Task {} submitted to queue {}",
            task_id, queue_name
        );

        // Notify about task submission
        let queue_event = EventPayload::queue_updated(&queue_name, project_id, worker_type, 1);
        self.event_broadcaster.broadcast(queue_event);

        let ticket_event = EventPayload::ticket_updated(ticket_id, project_id, "queued");
        self.event_broadcaster.broadcast(ticket_event);

        Ok(task_id)
    }

    /// Get existing queue sender or create new queue with consumer
    async fn get_or_create_queue(
        self: &Arc<Self>,
        queue_name: &str,
        project_id: &str,
        worker_type: &str,
    ) -> Result<mpsc::Sender<TaskItem>> {
        // Check if queue already exists
        if let Some(sender) = self.queues.get(queue_name) {
            trace!("[QueueManager] Using existing queue: {}", queue_name);
            return Ok(sender.clone());
        }

        // Create new queue and consumer
        info!(
            "[QueueManager] Creating new queue and consumer: {}",
            queue_name
        );
        let (sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER_SIZE);

        // Insert sender into map
        self.queues.insert(queue_name.to_string(), sender.clone());

        // Spawn consumer thread
        let queue_name_clone = queue_name.to_string();
        let project_id_clone = project_id.to_string();
        let worker_type_clone = worker_type.to_string();

        let queue_name_for_error = queue_name_clone.clone();
        let completion_sender = self.completion_sender.clone();
        let db_clone = self.db.clone();
        let _server_host = self.config.host.clone();
        let _server_port = self.config.port;
        let _permission_mode = self.config.permission_mode;

        let config_clone = self.config.clone();
        let event_broadcaster_clone = self.event_broadcaster.clone();

        tokio::spawn(async move {
            let db_for_cleanup = db_clone.clone();
            let event_broadcaster_for_cleanup = event_broadcaster_clone.clone();

            let consumer = Arc::new(WorkerConsumer::new(
                project_id_clone,
                worker_type_clone, // This becomes the 'stage' parameter in the new consumer
                config_clone,
                db_clone,
                completion_sender,
                event_broadcaster_clone,
            ));

            if let Err(e) = consumer.run(receiver).await {
                error!("Consumer failed for queue {}: {}", queue_name_for_error, e);

                // Emergency release of claimed tickets when consumer fails
                if let Err(release_error) = ClaimManager::emergency_release_claimed_tickets(&db_for_cleanup, &event_broadcaster_for_cleanup).await {
                    error!("Failed to emergency release tickets after consumer failure: {}", release_error);
                }
            }
        });

        debug!("[QueueManager] Started consumer for queue: {}", queue_name);

        // Notify about new queue creation
        let event = EventPayload::queue_updated(queue_name, project_id, worker_type, 0);
        self.event_broadcaster.broadcast(event);

        Ok(sender)
    }

    /// Get queue statistics (for monitoring)
    pub fn get_queue_count(&self) -> usize {
        self.queues.len()
    }

    /// List all active queue names
    pub fn list_queue_names(&self) -> Vec<String> {
        self.queues
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Start processing WorkerCompletionEvent messages with auto-enqueue
    pub async fn start_completion_event_processor(
        self: Arc<Self>,
        mut receiver: mpsc::Receiver<WorkerCompletionEvent>,
    ) {
        info!("[QueueManager] Starting WorkerCompletionEvent processor with auto-enqueue");

        while let Some(event) = receiver.recv().await {
            trace!(
                "[QueueManager] Processing WorkerCompletionEvent for ticket {:?}",
                event.ticket_id.as_str()
            );

            if let Err(e) = self.process_completion_event(&event).await {
                error!(
                    "[QueueManager] Failed to process WorkerCompletionEvent for ticket {}: {}",
                    event.ticket_id.as_str(),
                    e
                );
            }
        }

        info!("[QueueManager] WorkerCompletionEvent processor shut down");
    }

    async fn process_completion_event(
        self: &Arc<Self>,
        event: &WorkerCompletionEvent,
    ) -> Result<()> {
        info!(
            "Processing WorkerCompletionEvent for ticket {}: {:?}",
            event.ticket_id.as_str(),
            event.command
        );

        // Add worker comment
        crate::database::comments::Comment::create(
            &self.db,
            event.ticket_id.as_str(),
            Some("worker"),
            Some("system"),
            None,
            &event.comment,
        )
        .await?;

        match &event.command {
            WorkerCommand::AdvanceToStage {
                target_stage,
                pipeline_update,
            } => {
                // Handle stage advancement
                self.advance_ticket_to_stage(
                    &event.ticket_id,
                    target_stage,
                    pipeline_update.as_ref(),
                )
                .await?;

                // AUTO-ENQUEUE for next stage
                if let Err(e) = self
                    .auto_enqueue_ticket(event.ticket_id.as_str(), target_stage.as_str())
                    .await
                {
                    warn!(
                        "Failed to auto-enqueue ticket {} for stage {}: {}",
                        event.ticket_id.as_str(),
                        target_stage.as_str(),
                        e
                    );
                }
            }
            WorkerCommand::ReturnToStage {
                target_stage,
                reason,
            } => {
                self.return_ticket_to_stage(&event.ticket_id, target_stage, reason)
                    .await?;

                // AUTO-ENQUEUE for previous stage
                if let Err(e) = self
                    .auto_enqueue_ticket(event.ticket_id.as_str(), target_stage.as_str())
                    .await
                {
                    warn!(
                        "Failed to auto-enqueue ticket {} for stage {}: {}",
                        event.ticket_id.as_str(),
                        target_stage.as_str(),
                        e
                    );
                }
            }
            WorkerCommand::RequestCoordinatorAttention { reason } => {
                self.request_coordinator_attention(&event.ticket_id, reason)
                    .await?;
            }
        }

        // Handle dependency cascades after any completion event
        self.handle_dependency_cascade(event).await?;

        Ok(())
    }

    async fn auto_enqueue_ticket(
        self: &Arc<Self>,
        ticket_id: &str,
        target_stage: &str,
    ) -> Result<()> {
        // Get ticket to find project_id
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        let project_id = &ticket_with_comments.ticket.project_id;

        // Submit task to queue for the new stage
        match self.submit_task(project_id, target_stage, ticket_id).await {
            Ok(task_id) => {
                info!(
                    "Auto-enqueued ticket {} for stage {} (task_id: {})",
                    ticket_id, target_stage, task_id
                );
                Ok(())
            }
            Err(e) => {
                // This is expected for final stages or coordinator attention
                debug!(
                    "Could not auto-enqueue ticket {} for stage {}: {}",
                    ticket_id, target_stage, e
                );
                Ok(())
            }
        }
    }

    /// Handle advancing ticket to next stage with optional pipeline update
    async fn advance_ticket_to_stage(
        self: &Arc<Self>,
        ticket_id: &TicketId,
        target_stage: &WorkerType,
        pipeline_update: Option<&Vec<WorkerType>>,
    ) -> Result<()> {
        // Update pipeline FIRST if provided - this allows worker types to be created during planning
        if let Some(new_pipeline) = pipeline_update {
            self.update_pipeline(ticket_id, new_pipeline).await?;
        }

        // Validate that the target worker type exists in the project
        crate::validation::PipelineValidator::validate_worker_type_exists_for_ticket(
            &self.db,
            ticket_id.as_str(),
            target_stage.as_str(),
        )
        .await?;

        info!(
            "Moving ticket {} to next stage: {}",
            ticket_id.as_str(),
            target_stage.as_str()
        );

        self.transition_ticket_stage(ticket_id, target_stage).await
    }

    /// Handle returning ticket to previous stage
    async fn return_ticket_to_stage(
        self: &Arc<Self>,
        ticket_id: &TicketId,
        target_stage: &WorkerType,
        reason: &str,
    ) -> Result<()> {
        // Validate target stage
        crate::validation::PipelineValidator::validate_worker_type_exists_for_ticket(
            &self.db,
            ticket_id.as_str(),
            target_stage.as_str(),
        )
        .await?;

        warn!(
            "Moving ticket {} back to previous stage: {} (reason: {})",
            ticket_id.as_str(),
            target_stage.as_str(),
            reason
        );

        self.transition_ticket_stage(ticket_id, target_stage).await
    }

    /// Handle coordinator attention request
    async fn request_coordinator_attention(
        self: &Arc<Self>,
        ticket_id: &TicketId,
        reason: &str,
    ) -> Result<()> {
        warn!(
            "Ticket {} requires coordinator attention: {}",
            ticket_id.as_str(),
            reason
        );

        // Set ticket to on_hold
        crate::database::tickets::Ticket::update_state(
            &self.db,
            ticket_id.as_str(),
            &crate::database::tickets::TicketState::OnHold.to_string(),
        )
        .await?;

        // Create coordinator attention event
        crate::database::events::Event::create_stage_completed(
            &self.db,
            ticket_id.as_str(),
            "coordinator_attention",
            "system",
        )
        .await?;

        // Add special comment
        crate::database::comments::Comment::create(
            &self.db,
            ticket_id.as_str(),
            Some("system"),
            Some("system"),
            Some(999), // Special stage for system messages
            &format!("⚠️ COORDINATOR ATTENTION REQUIRED: {}", reason),
        )
        .await?;

        info!(
            "Set ticket {} to on_hold status for coordinator attention",
            ticket_id.as_str()
        );

        Ok(())
    }

    // Private helper methods
    async fn update_pipeline(
        self: &Arc<Self>,
        ticket_id: &TicketId,
        new_pipeline: &[WorkerType],
    ) -> Result<()> {
        info!(
            "Updating pipeline for ticket {} to: {:?}",
            ticket_id.as_str(),
            new_pipeline
        );

        // Get ticket to find project_id and current state for validation
        let ticket_with_comments =
            crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id.as_str())
                .await?
                .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id.as_str()))?;

        let ticket = &ticket_with_comments.ticket;
        let _project_id = &ticket.project_id;

        // Get original pipeline for past stage validation
        let original_pipeline = ticket.get_execution_plan()?;

        // Get current stage index for immutability validation
        let current_stage_index = self.get_current_stage_index(ticket)?;

        // Convert WorkerType to strings for validation and database
        let new_pipeline_strings: Vec<String> = new_pipeline
            .iter()
            .map(|wt| wt.as_str().to_string())
            .collect();

        // CRITICAL: Validate that past stages are preserved (immutable history)
        self.validate_pipeline_preserves_past_stages(
            &original_pipeline,
            &new_pipeline_strings,
            current_stage_index,
            ticket_id.as_str(),
        )?;

        // CRITICAL: Validate that ALL stages exist as worker types before database update
        crate::validation::PipelineValidator::validate_pipeline_stages(
            &self.db,
            &ticket.project_id,
            &new_pipeline_strings,
            "Pipeline update",
        )
        .await?;

        // Only proceed with database update if all validation passes
        let pipeline_json = serde_json::to_string(&new_pipeline_strings)
            .map_err(|e| anyhow::anyhow!("Failed to serialize pipeline: {}", e))?;

        sqlx::query(
            "UPDATE tickets SET execution_plan = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
        )
        .bind(pipeline_json)
        .bind(ticket_id.as_str())
        .execute(&self.db)
        .await?;

        info!(
            "Successfully updated pipeline for ticket {} (past {} stages preserved)",
            ticket_id.as_str(),
            current_stage_index + 1
        );
        Ok(())
    }

    /// Get the current stage index in the pipeline
    /// Returns the index of the current stage, or 0 for "planning" stage
    fn get_current_stage_index(&self, ticket: &crate::database::tickets::Ticket) -> Result<usize> {
        // Special case: planning stage is before the pipeline starts
        if ticket.current_stage == "planning" {
            return Ok(0);
        }

        // Use PipelineManager for actual pipeline logic
        PipelineManager::get_current_stage_index(ticket)
    }

    /// Validate that pipeline update preserves past stages (immutable history)
    /// Past stages (up to and including current stage) cannot be modified
    fn validate_pipeline_preserves_past_stages(
        &self,
        original_pipeline: &[String],
        new_pipeline: &[String],
        current_stage_index: usize,
        ticket_id: &str,
    ) -> Result<()> {
        // For planning stage, we allow full pipeline replacement since no stages are completed yet
        if current_stage_index == 0 {
            return Ok(());
        }

        // Verify that past stages (up to and including current stage) are preserved
        for i in 0..=current_stage_index {
            if i >= original_pipeline.len() {
                return Err(anyhow::anyhow!(
                    "Pipeline validation failed for ticket {}: original pipeline too short (index {} not found in pipeline of length {})",
                    ticket_id,
                    i,
                    original_pipeline.len()
                ));
            }

            if i >= new_pipeline.len() {
                return Err(anyhow::anyhow!(
                    "Pipeline validation failed for ticket {}: new pipeline truncates past stages (index {} not found in new pipeline of length {}). Past stages cannot be deleted.",
                    ticket_id,
                    i,
                    new_pipeline.len()
                ));
            }

            if original_pipeline[i] != new_pipeline[i] {
                return Err(anyhow::anyhow!(
                    "Pipeline validation failed for ticket {}: illegal modification of past stage at index {}: '{}' -> '{}'. Past stages are immutable.",
                    ticket_id,
                    i,
                    original_pipeline[i],
                    new_pipeline[i]
                ));
            }
        }

        info!(
            "Pipeline validation passed for ticket {}: past {} stages preserved",
            ticket_id,
            current_stage_index + 1
        );

        Ok(())
    }

    async fn transition_ticket_stage(
        self: &Arc<Self>,
        ticket_id: &TicketId,
        target_stage: &WorkerType,
    ) -> Result<()> {
        // Release ticket if claimed
        self.release_ticket_if_claimed(ticket_id).await?;

        // Update stage
        crate::database::tickets::Ticket::update_stage(
            &self.db,
            ticket_id.as_str(),
            target_stage.as_str(),
        )
        .await?;

        // Create completion event
        crate::database::events::Event::create_stage_completed(
            &self.db,
            ticket_id.as_str(),
            target_stage.as_str(),
            "system",
        )
        .await?;

        info!(
            "Successfully moved ticket {} to stage {}",
            ticket_id.as_str(),
            target_stage.as_str()
        );

        Ok(())
    }

    async fn release_ticket_if_claimed(self: &Arc<Self>, ticket_id: &TicketId) -> Result<()> {
        ClaimManager::release_ticket_if_claimed(&self.db, &self.event_broadcaster, ticket_id).await
    }

    /// Handle dependency cascades when tickets complete or advance stages
    async fn handle_dependency_cascade(
        self: &Arc<Self>,
        event: &WorkerCompletionEvent,
    ) -> Result<()> {
        let ticket_id = event.ticket_id.as_str();

        // Get the ticket to check its current state
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        let ticket = &ticket_with_comments.ticket;

        match &event.command {
            WorkerCommand::AdvanceToStage { .. } => {
                // When ticket advances, check if this unblocks any dependent tickets
                self.check_and_unblock_dependents(ticket_id).await?;

                // If this ticket has a parent, resubmit parent for reassessment
                if let Some(parent_id) = &ticket.parent_ticket_id {
                    self.resubmit_parent_ticket(parent_id).await?;
                }
            }
            WorkerCommand::RequestCoordinatorAttention { .. } => {
                // Parent may need to reassess strategy when child needs attention
                if let Some(parent_id) = &ticket.parent_ticket_id {
                    self.resubmit_parent_ticket(parent_id).await?;
                }
            }
            _ => {
                // For other commands, still check dependencies
                self.check_and_unblock_dependents(ticket_id).await?;
            }
        }

        Ok(())
    }

    /// Check if ticket completion unblocks any dependent tickets
    async fn check_and_unblock_dependents(
        self: &Arc<Self>,
        completed_ticket_id: &str,
    ) -> Result<()> {
        let ticket_id = TicketId::new(completed_ticket_id.to_string())?;
        DependencyManager::check_and_unblock_dependents(
            &self.db,
            &self.event_broadcaster,
            self.clone(),
            &ticket_id,
        )
        .await
    }

    /// Resubmit parent ticket when child completes or needs attention
    async fn resubmit_parent_ticket(self: &Arc<Self>, parent_ticket_id: &str) -> Result<()> {
        // Get parent ticket details
        if let Some(parent_with_comments) =
            crate::database::tickets::Ticket::get_by_id(&self.db, parent_ticket_id).await?
        {
            let parent_ticket = &parent_with_comments.ticket;

            // Only resubmit if parent is not already being processed and is open
            if parent_ticket.processing_worker_id.is_none() && parent_ticket.is_open() {
                info!(
                    "Resubmitting parent ticket {} at stage {} due to child activity",
                    parent_ticket_id, parent_ticket.current_stage
                );

                if let Err(e) = self
                    .auto_enqueue_ticket(parent_ticket_id, &parent_ticket.current_stage)
                    .await
                {
                    warn!(
                        "Failed to resubmit parent ticket {} for stage {}: {}",
                        parent_ticket_id, parent_ticket.current_stage, e
                    );
                } else {
                    info!(
                        "Successfully resubmitted parent ticket {} to stage {}",
                        parent_ticket_id, parent_ticket.current_stage
                    );
                }
            } else {
                debug!(
                    "Parent ticket {} not resubmitted (already processing: {}, state: {})",
                    parent_ticket_id,
                    parent_ticket.processing_worker_id.is_some(),
                    parent_ticket.state
                );
            }
        }

        Ok(())
    }
}

