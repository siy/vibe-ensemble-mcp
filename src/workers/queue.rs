use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::types::TaskItem;
use crate::database::DbPool;

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
    output_sender: mpsc::UnboundedSender<WorkerOutput>,
    #[allow(dead_code)]
    processor_handle: tokio::task::JoinHandle<()>,
}

impl Default for QueueManager {
    fn default() -> Self {
        panic!("QueueManager requires DbPool parameter - use QueueManager::new(db) instead of default()")
    }
}

impl QueueManager {
    pub fn new(db: DbPool) -> Self {
        let (output_sender, output_receiver) = mpsc::unbounded_channel();

        // Spawn the output processor thread
        let processor_handle = tokio::spawn(async move {
            Self::output_processor_loop(output_receiver, db).await;
        });

        Self {
            queues: DashMap::new(),
            output_sender,
            processor_handle,
        }
    }

    /// Get a sender for WorkerOutput processing
    pub fn get_output_sender(&self) -> mpsc::UnboundedSender<WorkerOutput> {
        self.output_sender.clone()
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
        let output_sender = self.output_sender.clone();
        let db_clone = _db.clone();

        tokio::spawn(async move {
            let consumer = WorkerConsumer::new(
                project_id_clone,
                worker_type_clone,
                queue_name_clone,
                receiver,
                output_sender,
                db_clone,
            );

            if let Err(e) = consumer.start().await {
                error!("Consumer failed for queue {}: {}", queue_name_for_error, e);
            }
        });

        info!("[QueueManager] Started consumer for queue: {}", queue_name);
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

    /// Output processor loop - handles WorkerOutput instances centrally
    /// This contains the validation and processing logic moved from json_output.rs
    async fn output_processor_loop(
        mut receiver: mpsc::UnboundedReceiver<WorkerOutput>,
        db: DbPool,
    ) {
        info!("[QueueManager] Starting centralized output processor");

        while let Some(output) = receiver.recv().await {
            trace!(
                "[OutputProcessor] Processing output for ticket {:?}",
                output.ticket_id
            );

            info!(
                "[OutputProcessor] Received output with outcome: {:?}, comment: {}",
                output.outcome, output.comment
            );

            // Process the WorkerOutput with full validation and database operations
            if let Err(e) = Self::process_worker_output(&db, &output).await {
                error!(
                    "[OutputProcessor] Failed to process output for ticket {:?}: {}",
                    output.ticket_id, e
                );
                // Continue processing other outputs even if one fails
            }

            info!(
                "[OutputProcessor] Completed processing for ticket {:?} with outcome {:?}",
                output.ticket_id, output.outcome
            );
        }

        info!("[QueueManager] Output processor shut down");
    }

    /// Process the parsed worker output and take appropriate actions
    /// This is the core logic moved from json_output.rs
    async fn process_worker_output(db: &DbPool, output: &WorkerOutput) -> Result<()> {
        let ticket_id = output
            .ticket_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkerOutput must have ticket_id filled"))?;

        info!(
            "Processing worker output for ticket {}: outcome={:?}, target_stage={:?}",
            ticket_id, output.outcome, output.target_stage
        );

        // First, validate that the ticket exists
        let ticket_exists = crate::database::tickets::Ticket::get_by_id(db, ticket_id)
            .await?
            .is_some();

        if !ticket_exists {
            return Err(anyhow::anyhow!("Ticket '{}' not found", ticket_id));
        }

        // Add the worker's comment
        crate::database::comments::Comment::create(
            db,
            ticket_id,
            Some("worker"), // Generic worker type since we don't have specific worker type here
            Some("system"), // Generic worker id since we don't track specific worker ids
            Some(1),        // Simple stage number
            &output.comment,
        )
        .await?;
        info!("Added worker comment for ticket {}", ticket_id);

        // Process based on outcome
        match output.outcome {
            WorkerOutcome::NextStage => {
                Self::handle_next_stage(db, ticket_id, output).await?;
            }
            WorkerOutcome::PrevStage => {
                Self::handle_prev_stage(db, ticket_id, output).await?;
            }
            WorkerOutcome::CoordinatorAttention => {
                Self::handle_coordinator_attention(db, ticket_id, output).await?;
            }
        }

        Ok(())
    }

    async fn handle_next_stage(db: &DbPool, ticket_id: &str, output: &WorkerOutput) -> Result<()> {
        let target_stage = output
            .target_stage
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("next_stage outcome requires target_stage"))?;

        // Update pipeline FIRST if provided - this allows worker types to be created during planning
        if let Some(new_pipeline) = &output.pipeline_update {
            info!(
                "Updating pipeline for ticket {} to: {:?}",
                ticket_id, new_pipeline
            );

            // Get ticket to find project_id for pipeline validation
            let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(db, ticket_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

            // Validate all stages in the new pipeline have registered worker types
            for stage in new_pipeline {
                let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
                    db,
                    &ticket_with_comments.ticket.project_id,
                    stage,
                )
                .await?
                .is_some();

                if !worker_type_exists {
                    return Err(anyhow::anyhow!(
                        "Unknown worker type '{}'. Please, register '{}' first.",
                        stage,
                        stage
                    ));
                }
            }

            // Update the ticket's execution_plan (pipeline)
            let pipeline_json = serde_json::to_string(new_pipeline)?;
            sqlx::query(
                "UPDATE tickets SET execution_plan = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
            )
            .bind(&pipeline_json)
            .bind(ticket_id)
            .execute(db)
            .await?;
        }

        // Now validate that the target stage has a registered worker type
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
            db,
            &ticket_with_comments.ticket.project_id,
            target_stage,
        )
        .await?
        .is_some();

        if !worker_type_exists {
            return Err(anyhow::anyhow!(
                "Unknown worker type '{}'. Please, register '{}' first.",
                target_stage,
                target_stage
            ));
        }

        info!(
            "Moving ticket {} to next stage: {}",
            ticket_id, target_stage
        );

        // Release ticket from current worker (if claimed)
        Self::release_ticket_if_claimed(db, ticket_id).await?;

        // Move ticket to target stage
        crate::database::tickets::Ticket::update_stage(db, ticket_id, target_stage).await?;

        // Create an event for successful stage completion
        crate::database::events::Event::create_stage_completed(
            db,
            ticket_id,
            target_stage,
            "system",
        )
        .await?;

        info!(
            "Successfully moved ticket {} to stage {}",
            ticket_id, target_stage
        );
        Ok(())
    }

    async fn handle_prev_stage(db: &DbPool, ticket_id: &str, output: &WorkerOutput) -> Result<()> {
        let target_stage = output
            .target_stage
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("prev_stage outcome requires target_stage"))?;

        // Validate that the target stage has a registered worker type
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
            db,
            &ticket_with_comments.ticket.project_id,
            target_stage,
        )
        .await?
        .is_some();

        if !worker_type_exists {
            return Err(anyhow::anyhow!(
                "Unknown worker type '{}'. Please, register '{}' first.",
                target_stage,
                target_stage
            ));
        }

        warn!(
            "Moving ticket {} back to previous stage: {} (reason: {})",
            ticket_id, target_stage, output.reason
        );

        // Release ticket from current worker (if claimed)
        Self::release_ticket_if_claimed(db, ticket_id).await?;

        // Move ticket back to target stage
        crate::database::tickets::Ticket::update_stage(db, ticket_id, target_stage).await?;

        // Create an event for tracking
        crate::database::events::Event::create_stage_completed(
            db,
            ticket_id,
            target_stage,
            "system",
        )
        .await?;

        info!(
            "Successfully moved ticket {} back to stage {}",
            ticket_id, target_stage
        );
        Ok(())
    }

    async fn handle_coordinator_attention(
        db: &DbPool,
        ticket_id: &str,
        output: &WorkerOutput,
    ) -> Result<()> {
        warn!(
            "Ticket {} requires coordinator attention: {}",
            ticket_id, output.reason
        );

        // Set ticket state to on_hold to signal coordinator intervention needed
        crate::database::tickets::Ticket::update_state(db, ticket_id, "on_hold").await?;

        // Create an event for coordinator attention
        crate::database::events::Event::create_stage_completed(
            db,
            ticket_id,
            "coordinator_attention",
            "system",
        )
        .await?;

        // Add a special comment indicating coordinator attention is needed
        crate::database::comments::Comment::create(
            db,
            ticket_id,
            Some("system"),
            Some("system"),
            Some(999), // Special stage for system messages
            &format!("⚠️ COORDINATOR ATTENTION REQUIRED: {}", output.reason),
        )
        .await?;

        info!(
            "Set ticket {} to on_hold status for coordinator attention",
            ticket_id
        );
        Ok(())
    }

    /// Release a ticket if it's currently claimed by any worker
    async fn release_ticket_if_claimed(db: &DbPool, ticket_id: &str) -> Result<()> {
        debug!("Releasing ticket {} if claimed", ticket_id);

        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL
            "#,
        )
        .bind(ticket_id)
        .execute(db)
        .await?;

        if result.rows_affected() > 0 {
            info!("Released claimed ticket {} for stage transition", ticket_id);
        } else {
            debug!("Ticket {} was not claimed, no release needed", ticket_id);
        }

        Ok(())
    }
}

/// Simplified consumer that processes tasks from mpsc channel
struct WorkerConsumer {
    project_id: String,
    worker_type: String,
    queue_name: String,
    receiver: mpsc::UnboundedReceiver<TaskItem>,
    output_sender: mpsc::UnboundedSender<WorkerOutput>,
    db: DbPool,
}

impl WorkerConsumer {
    fn new(
        project_id: String,
        worker_type: String,
        queue_name: String,
        receiver: mpsc::UnboundedReceiver<TaskItem>,
        output_sender: mpsc::UnboundedSender<WorkerOutput>,
        db: DbPool,
    ) -> Self {
        Self {
            project_id,
            worker_type,
            queue_name,
            receiver,
            output_sender,
            db,
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

        // Send output to centralized processor
        if self.output_sender.send(worker_output).is_err() {
            warn!(
                "Output processor has shut down, cannot send output for ticket {}",
                task.ticket_id
            );
        }

        Ok(())
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
        .ok_or_else(|| anyhow::anyhow!(
            "Worker type '{}' not found for project '{}'",
            self.worker_type,
            self.project_id
        ))?;

        let spawn_request = SpawnWorkerRequest {
            worker_id,
            project_id: self.project_id.clone(),
            worker_type: self.worker_type.clone(),
            queue_name: self.queue_name.clone(),
            ticket_id: ticket_id.to_string(),
            project_path: project.path,
            system_prompt: worker_type_info.system_prompt,
            server_port: 3000, // TODO: Get from configuration
        };

        ProcessManager::spawn_worker(spawn_request).await
    }
}
