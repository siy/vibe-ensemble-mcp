use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::fmt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::types::TaskItem;
use crate::{
    config::Config,
    database::DbPool,
    sse::{notify_queue_change, notify_ticket_change, EventBroadcaster},
    workers::{
        domain::{TicketId, WorkerCommand, WorkerCompletionEvent, WorkerType},
        output::OutputProcessor,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
    pub ticket_id: Option<String>,
    pub outcome: WorkerOutcome,
    pub target_stage: Option<String>,
    pub pipeline_update: Option<Vec<String>>,
    pub comment: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    NextStage,
    PrevStage,
    CoordinatorAttention,
}

pub struct QueueManager {
    queues: DashMap<String, mpsc::UnboundedSender<TaskItem>>,
    completion_sender: mpsc::UnboundedSender<WorkerCompletionEvent>,
    worker_output_sender: mpsc::UnboundedSender<WorkerOutput>,
    config: Config,
    event_broadcaster: EventBroadcaster,
}

impl Default for QueueManager {
    fn default() -> Self {
        panic!("QueueManager requires DbPool and Config parameters - use QueueManager::new(db, config) instead of default()")
    }
}

impl fmt::Debug for QueueManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueManager")
            .field("queue_count", &self.queues.len())
            .finish()
    }
}

impl QueueManager {
    pub fn new(
        db: DbPool,
        config: Config,
        event_broadcaster: EventBroadcaster,
    ) -> (Self, mpsc::UnboundedReceiver<WorkerOutput>) {
        let (completion_sender, completion_receiver) = mpsc::unbounded_channel();
        let (worker_output_sender, worker_output_receiver) = mpsc::unbounded_channel();

        // Start event processor independently (for WorkerCompletionEvent)
        let output_processor = OutputProcessor::new(db.clone());
        tokio::spawn(async move {
            output_processor
                .start_event_processing(completion_receiver)
                .await;
        });

        let queue_manager = Self {
            queues: DashMap::new(),
            completion_sender,
            worker_output_sender,
            config,
            event_broadcaster,
        };

        // Return both the QueueManager and the WorkerOutput receiver
        // The caller will start the WorkerOutput processor after wrapping in Arc
        (queue_manager, worker_output_receiver)
    }

    /// Get a sender for WorkerCompletionEvent processing
    pub fn get_completion_sender(&self) -> mpsc::UnboundedSender<WorkerCompletionEvent> {
        self.completion_sender.clone()
    }

    /// Get a sender for WorkerOutput processing (legacy format)
    pub fn get_worker_output_sender(&self) -> mpsc::UnboundedSender<WorkerOutput> {
        self.worker_output_sender.clone()
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
        db: &DbPool,
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
        let worker_type_exists =
            crate::database::worker_types::WorkerType::get_by_type(db, project_id, worker_type)
                .await?;

        if worker_type_exists.is_none() {
            return Err(anyhow::anyhow!(
                "Worker type '{}' does not exist for project '{}'. Cannot submit task for ticket {}",
                worker_type,
                project_id,
                ticket_id
            ));
        }

        // Claim the ticket before submitting to queue
        let worker_id = format!("consumer-{}-{}", worker_type, &task_id[..8]);
        let claim_result =
            crate::database::tickets::Ticket::claim_for_processing(db, ticket_id, &worker_id)
                .await?;

        if claim_result == 0 {
            return Err(anyhow::anyhow!(
                "Ticket {} already claimed by another worker",
                ticket_id
            ));
        }

        info!(
            "[QueueManager] Claimed ticket {} with worker {}",
            ticket_id, worker_id
        );

        // Notify about ticket being claimed for processing
        notify_ticket_change(&self.event_broadcaster, ticket_id, "claimed").await;

        let task = TaskItem {
            task_id: task_id.clone(),
            ticket_id: ticket_id.to_string(),
            created_at: chrono::Utc::now(),
        };

        // Get or create queue with consumer
        let sender = self
            .get_or_create_queue(&queue_name, project_id, worker_type, db)
            .await?;

        // Send task to queue
        if sender.send(task).is_err() {
            return Err(anyhow::anyhow!("Queue {} is closed", queue_name));
        }

        info!(
            "[QueueManager] Task {} submitted to queue {}",
            task_id, queue_name
        );

        // Notify about task submission
        notify_queue_change(&self.event_broadcaster, &queue_name, "task_submitted").await;
        notify_ticket_change(&self.event_broadcaster, ticket_id, "queued").await;

        Ok(task_id)
    }

    /// Get existing queue sender or create new queue with consumer
    async fn get_or_create_queue(
        self: &Arc<Self>,
        queue_name: &str,
        project_id: &str,
        worker_type: &str,
        _db: &DbPool,
    ) -> Result<mpsc::UnboundedSender<TaskItem>> {
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
        let (sender, receiver) = mpsc::unbounded_channel();

        // Insert sender into map
        self.queues.insert(queue_name.to_string(), sender.clone());

        // Spawn consumer thread
        let queue_name_clone = queue_name.to_string();
        let project_id_clone = project_id.to_string();
        let worker_type_clone = worker_type.to_string();

        let queue_name_for_error = queue_name_clone.clone();
        let completion_sender = self.completion_sender.clone();
        let db_clone = _db.clone();
        let server_port = self.config.port;
        let permission_mode = self.config.permission_mode.clone();

        // Clone these for emergency release (after they're moved to consumer)
        let emergency_db = db_clone.clone();
        let emergency_project_id = project_id_clone.clone();
        let emergency_worker_type = worker_type_clone.clone();

        tokio::spawn(async move {
            let consumer = WorkerConsumer::new(
                project_id_clone,
                worker_type_clone,
                queue_name_clone,
                receiver,
                completion_sender,
                db_clone,
                server_port,
                permission_mode,
            );

            if let Err(e) = consumer.start().await {
                error!("Consumer failed for queue {}: {}", queue_name_for_error, e);
                // Critical: Release any claimed tickets for this queue when thread fails entirely
                if let Err(release_err) = WorkerConsumer::emergency_release_claimed_tickets(
                    &emergency_db,
                    &emergency_project_id,
                    &emergency_worker_type,
                )
                .await
                {
                    error!(
                        "Failed to release claimed tickets after consumer failure: {}",
                        release_err
                    );
                }
            }
        });

        info!("[QueueManager] Started consumer for queue: {}", queue_name);

        // Notify about new queue creation
        notify_queue_change(&self.event_broadcaster, queue_name, "created").await;

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

    // Removed old static process_worker_output function - now using instance method instead

    /// Start processing WorkerOutput messages with auto-enqueue
    pub async fn start_worker_output_processor(
        self: Arc<Self>,
        db: DbPool,
        mut receiver: mpsc::UnboundedReceiver<WorkerOutput>,
    ) {
        info!("[QueueManager] Starting WorkerOutput processor with auto-enqueue");

        while let Some(output) = receiver.recv().await {
            trace!(
                "[QueueManager] Processing WorkerOutput for ticket {:?}",
                output.ticket_id
            );

            if let Err(e) = self.process_worker_output(&db, &output).await {
                error!(
                    "[QueueManager] Failed to process WorkerOutput for ticket {:?}: {}",
                    output.ticket_id, e
                );
            }
        }

        info!("[QueueManager] WorkerOutput processor shut down");
    }

    async fn process_worker_output(
        self: &Arc<Self>,
        db: &DbPool,
        output: &WorkerOutput,
    ) -> Result<()> {
        use crate::workers::{domain::*, output::handlers::OutputHandlers};

        let ticket_id_str = output
            .ticket_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkerOutput missing ticket_id"))?;

        info!(
            "Processing WorkerOutput for ticket {}: {:?} -> {:?}",
            ticket_id_str, output.outcome, output.target_stage
        );

        // Add worker comment
        crate::database::comments::Comment::create(
            db,
            ticket_id_str,
            Some("worker"),
            Some("system"),
            None,
            &output.comment,
        )
        .await?;

        // Create handlers instance for processing
        let handlers = OutputHandlers::new(db.clone());

        // Convert string ticket_id to TicketId domain type
        let ticket_id = TicketId::new(ticket_id_str.clone())?;

        match output.outcome {
            WorkerOutcome::NextStage => {
                let target_stage_str = output.target_stage.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Missing target_stage for next_stage outcome")
                })?;
                let target_stage = WorkerType::new(target_stage_str.clone())?;

                // Convert pipeline update to domain types if provided
                let pipeline_update = output.pipeline_update.as_ref().map(|pipeline| {
                    pipeline
                        .iter()
                        .filter_map(|s| WorkerType::new(s.clone()).ok())
                        .collect::<Vec<_>>()
                });

                // Handle stage advancement
                handlers
                    .handle_advance_to_stage(&ticket_id, &target_stage, pipeline_update.as_ref())
                    .await?;

                // AUTO-ENQUEUE for next stage
                if let Err(e) = self
                    .auto_enqueue_ticket(db, ticket_id_str, target_stage_str)
                    .await
                {
                    warn!(
                        "Failed to auto-enqueue ticket {} for stage {}: {}",
                        ticket_id_str, target_stage_str, e
                    );
                }
            }
            WorkerOutcome::PrevStage => {
                let target_stage_str = output.target_stage.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Missing target_stage for prev_stage outcome")
                })?;
                let target_stage = WorkerType::new(target_stage_str.clone())?;

                handlers
                    .handle_return_to_stage(&ticket_id, &target_stage, &output.reason)
                    .await?;

                // AUTO-ENQUEUE for previous stage
                if let Err(e) = self
                    .auto_enqueue_ticket(db, ticket_id_str, target_stage_str)
                    .await
                {
                    warn!(
                        "Failed to auto-enqueue ticket {} for stage {}: {}",
                        ticket_id_str, target_stage_str, e
                    );
                }
            }
            WorkerOutcome::CoordinatorAttention => {
                handlers
                    .handle_coordinator_attention(&ticket_id, &output.reason)
                    .await?;
            }
        }

        Ok(())
    }

    async fn auto_enqueue_ticket(
        self: &Arc<Self>,
        db: &DbPool,
        ticket_id: &str,
        target_stage: &str,
    ) -> Result<()> {
        // Get ticket to find project_id
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        let project_id = &ticket_with_comments.ticket.project_id;

        // Submit task to queue for the new stage
        match self
            .submit_task(project_id, target_stage, ticket_id, db)
            .await
        {
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
}

/// Simplified consumer that processes tasks from mpsc channel
struct WorkerConsumer {
    project_id: String,
    worker_type: String,
    queue_name: String,
    receiver: mpsc::UnboundedReceiver<TaskItem>,
    completion_sender: mpsc::UnboundedSender<WorkerCompletionEvent>,
    db: DbPool,
    server_port: u16,
    permission_mode: String,
}

impl WorkerConsumer {
    #[allow(clippy::too_many_arguments)]
    fn new(
        project_id: String,
        worker_type: String,
        queue_name: String,
        receiver: mpsc::UnboundedReceiver<TaskItem>,
        completion_sender: mpsc::UnboundedSender<WorkerCompletionEvent>,
        db: DbPool,
        server_port: u16,
        permission_mode: String,
    ) -> Self {
        Self {
            project_id,
            worker_type,
            queue_name,
            receiver,
            completion_sender,
            db,
            server_port,
            permission_mode,
        }
    }

    async fn start(mut self) -> Result<()> {
        info!(
            "[WorkerConsumer] Starting consumer for {}-{} (queue: {})",
            self.project_id, self.worker_type, self.queue_name
        );

        while let Some(task) = self.receiver.recv().await {
            trace!(
                "[WorkerConsumer] Received task {} for ticket {}",
                task.task_id,
                task.ticket_id
            );

            if let Err(e) = self.process_task(&task).await {
                error!("Failed to process task {}: {}", task.task_id, e);
                // Continue processing other tasks even if one fails
            }
        }

        info!(
            "[WorkerConsumer] Consumer shut down for queue: {}",
            self.queue_name
        );
        Ok(())
    }

    async fn process_task(&self, task: &TaskItem) -> Result<()> {
        trace!(
            "[WorkerConsumer] Processing task {} for ticket {}",
            task.task_id,
            task.ticket_id
        );

        info!(
            "[WorkerConsumer] Processing ticket {} with worker type {}",
            task.ticket_id, self.worker_type
        );

        // Execute worker and handle all 3 cases
        let worker_output = match self.execute_worker(&task.ticket_id).await {
            Ok(mut output) => {
                // Case 1: Success - fill ticket_id and pass through
                output.ticket_id = Some(task.ticket_id.clone());
                info!(
                    "Worker completed successfully for ticket {} with outcome {:?}",
                    task.ticket_id, output.outcome
                );
                output
            }
            Err(e) => {
                // Case 2: Worker crash or Case 3: Internal error
                // Both handled as CoordinatorAttention
                error!("Worker failed for ticket {}: {}", task.ticket_id, e);

                WorkerOutput {
                    ticket_id: Some(task.ticket_id.clone()),
                    outcome: crate::workers::queue::WorkerOutcome::CoordinatorAttention,
                    target_stage: None,
                    pipeline_update: None,
                    comment: format!(
                        "Worker {} failed for ticket {}",
                        self.worker_type, task.ticket_id
                    ),
                    reason: format!(
                        "Worker execution failed: {}. Worker type: {}, Queue: {}",
                        e, self.worker_type, self.queue_name
                    ),
                }
            }
        };

        // Convert WorkerOutput to WorkerCompletionEvent
        let completion_event = self.convert_to_completion_event(&worker_output)?;

        // Send event to centralized processor
        if self.completion_sender.send(completion_event).is_err() {
            warn!(
                "Output processor has shut down, cannot send completion event for ticket {}",
                task.ticket_id
            );
        }

        Ok(())
    }

    /// Convert WorkerOutput to WorkerCompletionEvent
    fn convert_to_completion_event(&self, output: &WorkerOutput) -> Result<WorkerCompletionEvent> {
        let ticket_id_str = output
            .ticket_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkerOutput must have ticket_id"))?;

        let ticket_id = TicketId::new(ticket_id_str.clone())?;

        // Convert WorkerOutcome to WorkerCommand
        let command = match output.outcome {
            WorkerOutcome::NextStage => {
                if let Some(ref target_stage_str) = output.target_stage {
                    let target_stage = WorkerType::new(target_stage_str.clone())?;
                    let pipeline_update = output.pipeline_update.as_ref().map(|pipeline| {
                        pipeline
                            .iter()
                            .filter_map(|s| WorkerType::new(s.clone()).ok())
                            .collect()
                    });

                    WorkerCommand::AdvanceToStage {
                        target_stage,
                        pipeline_update,
                    }
                } else {
                    return Err(anyhow::anyhow!("NextStage outcome requires target_stage"));
                }
            }
            WorkerOutcome::PrevStage => {
                if let Some(ref target_stage_str) = output.target_stage {
                    let target_stage = WorkerType::new(target_stage_str.clone())?;
                    WorkerCommand::ReturnToStage {
                        target_stage,
                        reason: output.reason.clone(),
                    }
                } else {
                    return Err(anyhow::anyhow!("PrevStage outcome requires target_stage"));
                }
            }
            WorkerOutcome::CoordinatorAttention => WorkerCommand::RequestCoordinatorAttention {
                reason: output.reason.clone(),
            },
        };

        Ok(WorkerCompletionEvent {
            ticket_id,
            command,
            comment: output.comment.clone(),
        })
    }

    /// Execute actual worker process - spawn Claude Code worker using ProcessManager
    async fn execute_worker(&self, ticket_id: &str) -> Result<WorkerOutput> {
        use crate::workers::process::ProcessManager;
        use crate::workers::types::SpawnWorkerRequest;
        use uuid::Uuid;

        info!(
            "[execute_worker] Processing ticket {} with worker type {}",
            ticket_id, self.worker_type
        );

        // Gather required data for SpawnWorkerRequest from database
        let worker_id = format!("worker-{}", Uuid::new_v4());

        // Get project path from database
        let project = crate::database::projects::Project::get_by_name(&self.db, &self.project_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", self.project_id))?;

        // Get system prompt from worker type in database
        let worker_type_info = crate::database::worker_types::WorkerType::get_by_type(
            &self.db,
            &self.project_id,
            &self.worker_type,
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Worker type '{}' not found for project '{}'",
                self.worker_type,
                self.project_id
            )
        })?;

        let spawn_request = SpawnWorkerRequest {
            worker_id,
            project_id: self.project_id.clone(),
            worker_type: self.worker_type.clone(),
            queue_name: self.queue_name.clone(),
            ticket_id: ticket_id.to_string(),
            project_path: project.path,
            system_prompt: worker_type_info.system_prompt,
            server_port: self.server_port,
            permission_mode: self.permission_mode.clone(),
        };

        ProcessManager::spawn_worker(spawn_request).await
    }

    /// Emergency function to release all claimed tickets for a specific worker type when consumer thread fails
    async fn emergency_release_claimed_tickets(
        db: &DbPool,
        project_id: &str,
        worker_type: &str,
    ) -> Result<()> {
        warn!(
            "Emergency releasing claimed tickets for project={}, worker_type={}",
            project_id, worker_type
        );

        // Release all tickets claimed by workers with matching prefix for this worker type
        let worker_prefix = format!("consumer-{}-", worker_type);

        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE project_id = ?1 
              AND current_stage = ?2 
              AND processing_worker_id IS NOT NULL 
              AND processing_worker_id LIKE ?3
            "#,
        )
        .bind(project_id)
        .bind(worker_type)
        .bind(format!("{}%", worker_prefix))
        .execute(db)
        .await?;

        if result.rows_affected() > 0 {
            error!(
                "Emergency released {} claimed tickets for project={}, worker_type={} due to consumer thread failure",
                result.rows_affected(),
                project_id,
                worker_type
            );

            // Add system comments to released tickets explaining what happened
            let released_tickets = sqlx::query(
                r#"
                SELECT ticket_id FROM tickets 
                WHERE project_id = ?1 
                  AND current_stage = ?2 
                  AND processing_worker_id IS NULL
                  AND updated_at >= datetime('now', '-5 seconds')
                "#,
            )
            .bind(project_id)
            .bind(worker_type)
            .fetch_all(db)
            .await?;

            for ticket_row in released_tickets {
                let ticket_id: String = ticket_row.get("ticket_id");
                let _ = crate::database::comments::Comment::create(
                    db,
                    &ticket_id,
                    Some("system"),
                    Some("system"),
                    Some(999), // Special stage for system messages
                    &format!(
                        "🚨 EMERGENCY RELEASE: Consumer thread for worker type '{}' failed. Ticket released for manual intervention or retry.",
                        worker_type
                    ),
                ).await;
            }
        } else {
            info!(
                "No claimed tickets found to release for project={}, worker_type={}",
                project_id, worker_type
            );
        }

        Ok(())
    }
}
