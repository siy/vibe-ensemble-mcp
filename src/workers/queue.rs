use anyhow::Result;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::types::TaskItem;
use crate::database::DbPool;

pub struct QueueManager {
    queues: DashMap<String, mpsc::UnboundedSender<TaskItem>>,
}

impl Default for QueueManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
        }
    }

    /// Generate standardized queue name: "{project_id}-{worker_type}-queue"
    pub fn generate_queue_name(project_id: &str, worker_type: &str) -> String {
        format!("{}-{}-queue", project_id, worker_type)
    }

    /// Submit task to worker queue - creates queue and spawns consumer if needed
    pub async fn submit_task(
        &self,
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
        &self,
        queue_name: &str,
        project_id: &str,
        worker_type: &str,
        db: &DbPool,
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

        let db_clone = db.clone();
        let queue_name_for_error = queue_name_clone.clone();
        tokio::spawn(async move {
            let consumer = WorkerConsumer::new(
                project_id_clone,
                worker_type_clone,
                queue_name_clone,
                receiver,
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
}

/// Simplified consumer that processes tasks from mpsc channel
struct WorkerConsumer {
    project_id: String,
    worker_type: String,
    queue_name: String,
    receiver: mpsc::UnboundedReceiver<TaskItem>,
    db: DbPool,
}

impl WorkerConsumer {
    fn new(
        project_id: String,
        worker_type: String,
        queue_name: String,
        receiver: mpsc::UnboundedReceiver<TaskItem>,
        db: DbPool,
    ) -> Self {
        Self {
            project_id,
            worker_type,
            queue_name,
            receiver,
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

        // Claim the ticket
        if !self.claim_ticket(&task.ticket_id).await? {
            debug!(
                "Ticket {} already claimed by another worker",
                task.ticket_id
            );
            return Ok(());
        }

        info!(
            "[WorkerConsumer] Processing ticket {} with worker type {}",
            task.ticket_id, self.worker_type
        );

        // For now, use placeholder worker logic
        let worker_result = self.simulate_worker(&task.ticket_id).await;

        match worker_result {
            Ok(output) => {
                info!(
                    "Worker completed successfully for ticket {}",
                    task.ticket_id
                );
                self.handle_worker_success(&task.ticket_id, &output).await?;
            }
            Err(e) => {
                error!("Worker failed for ticket {}: {}", task.ticket_id, e);
                self.handle_worker_failure(&task.ticket_id, &e.to_string())
                    .await?;
            }
        }

        Ok(())
    }

    async fn claim_ticket(&self, ticket_id: &str) -> Result<bool> {
        use crate::database::tickets::Ticket;

        let worker_id = format!(
            "consumer-{}-{}",
            self.worker_type,
            &Uuid::new_v4().to_string()[..8]
        );

        trace!(
            "[WorkerConsumer] Attempting to claim ticket {} with worker {}",
            ticket_id,
            worker_id
        );
        let updated = Ticket::claim_for_processing(&self.db, ticket_id, &worker_id).await?;

        Ok(updated > 0)
    }

    async fn simulate_worker(&self, ticket_id: &str) -> Result<WorkerOutput> {
        // Placeholder worker implementation
        info!(
            "Simulating worker for ticket {} - placeholder implementation",
            ticket_id
        );

        Ok(WorkerOutput {
            outcome: "next_stage".to_string(),
            target_stage: Some("completed".to_string()),
            comment: format!(
                "Processed ticket {} with worker type {}",
                ticket_id, self.worker_type
            ),
            reason: "Placeholder implementation".to_string(),
        })
    }

    async fn handle_worker_success(&self, ticket_id: &str, output: &WorkerOutput) -> Result<()> {
        use crate::database::tickets::Ticket;

        match output.outcome.as_str() {
            "next_stage" => {
                if let Some(target_stage) = &output.target_stage {
                    trace!(
                        "[WorkerConsumer] Transitioning ticket {} to stage {}",
                        ticket_id,
                        target_stage
                    );

                    // Update ticket stage
                    Ticket::update_stage(&self.db, ticket_id, target_stage).await?;

                    // Add comment
                    crate::database::comments::Comment::create(
                        &self.db,
                        ticket_id,
                        Some(&self.worker_type),
                        None,
                        None,
                        &format!("Stage completed: {}", output.comment),
                    )
                    .await?;

                    // Submit to next stage queue
                    // Note: Cannot submit to next stage from here without QueueManager reference
                    // This will need to be handled differently in the new architecture

                    info!(
                        "[WorkerConsumer] Ticket {} moved to stage {}",
                        ticket_id, target_stage
                    );
                }
            }
            "coordinator_attention" => {
                trace!(
                    "[WorkerConsumer] Processing coordinator_attention outcome for ticket {}",
                    ticket_id
                );
                
                // Put ticket on hold
                Ticket::update_state(&self.db, ticket_id, "on_hold").await?;
                
                // Create coordinator attention event using the reason field
                crate::database::events::Event::create(
                    &self.db,
                    "coordinator_attention",
                    Some(ticket_id),
                    None,
                    Some(&self.worker_type),
                    Some(&output.reason),
                ).await?;
                
                // Add comment about coordinator attention
                crate::database::comments::Comment::create(
                    &self.db,
                    ticket_id,
                    Some(&self.worker_type),
                    None,
                    None,
                    &format!("Coordinator attention required: {}", output.comment),
                )
                .await?;
                
                warn!(
                    "Ticket {} requires coordinator attention: {}",
                    ticket_id, output.reason
                );
            }
            _ => {
                warn!("Unknown worker outcome: {}", output.outcome);
            }
        }

        Ok(())
    }

    async fn handle_worker_failure(&self, ticket_id: &str, error: &str) -> Result<()> {
        use crate::database::tickets::Ticket;

        // Put ticket on hold
        Ticket::update_state(&self.db, ticket_id, "on_hold").await?;

        // Create event
        crate::database::events::Event::create(
            &self.db,
            "worker_failure",
            Some(ticket_id),
            None,
            Some(&self.worker_type),
            Some(error),
        )
        .await?;

        warn!(
            "Ticket {} put on hold due to worker failure: {}",
            ticket_id, error
        );
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct WorkerOutput {
    pub outcome: String,
    pub target_stage: Option<String>,
    pub comment: String,
    pub reason: String,
}
