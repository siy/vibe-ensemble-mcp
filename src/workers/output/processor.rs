use super::handlers::OutputHandlers;
use crate::workers::domain::*;
use crate::workers::queue::WorkerOutput; // Legacy type for compatibility
use crate::database::DbPool;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info, trace};

pub struct OutputProcessor {
    db: DbPool,
    handlers: OutputHandlers,
}

impl OutputProcessor {
    pub fn new(db: DbPool) -> Self {
        Self {
            db: db.clone(),
            handlers: OutputHandlers::new(db),
        }
    }

    /// Start processing WorkerCompletionEvent messages (new format)
    pub async fn start_event_processing(
        &self,
        mut receiver: mpsc::UnboundedReceiver<WorkerCompletionEvent>,
    ) {
        info!("[OutputProcessor] Starting centralized event processor (new format)");

        while let Some(event) = receiver.recv().await {
            trace!(
                "[OutputProcessor] Processing completion event for ticket {:?}",
                event.ticket_id
            );

            if let Err(e) = self.process_completion_event(&event).await {
                error!(
                    "[OutputProcessor] Failed to process completion event for ticket {:?}: {}",
                    event.ticket_id, e
                );
            }
        }

        info!("[OutputProcessor] Event processor shut down");
    }

    /// Start processing legacy WorkerOutput messages (backward compatibility)
    pub async fn start_legacy_processing(
        &self,
        mut receiver: mpsc::UnboundedReceiver<WorkerOutput>,
    ) {
        info!("[OutputProcessor] Starting centralized output processor (legacy format)");

        while let Some(output) = receiver.recv().await {
            trace!(
                "[OutputProcessor] Processing legacy output for ticket {:?}",
                output.ticket_id
            );

            if let Err(e) = self.process_legacy_worker_output(&output).await {
                error!(
                    "[OutputProcessor] Failed to process legacy output for ticket {:?}: {}",
                    output.ticket_id, e
                );
            }
        }

        info!("[OutputProcessor] Legacy output processor shut down");
    }

    async fn process_completion_event(&self, event: &WorkerCompletionEvent) -> Result<()> {
        // Add worker comment first
        self.add_worker_comment(&event.ticket_id, &event.comment).await?;

        // Process the command
        match &event.command {
            WorkerCommand::AdvanceToStage { target_stage, pipeline_update } => {
                self.handlers.handle_advance_to_stage(
                    &event.ticket_id, 
                    target_stage, 
                    pipeline_update.as_ref()
                ).await
            }
            WorkerCommand::ReturnToStage { target_stage, reason } => {
                self.handlers.handle_return_to_stage(
                    &event.ticket_id, 
                    target_stage, 
                    reason
                ).await
            }
            WorkerCommand::RequestCoordinatorAttention { reason } => {
                self.handlers.handle_coordinator_attention(
                    &event.ticket_id, 
                    reason
                ).await
            }
        }
    }

    /// Process legacy WorkerOutput format for backward compatibility
    async fn process_legacy_worker_output(&self, output: &WorkerOutput) -> Result<()> {
        let ticket_id = output
            .ticket_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkerOutput must have ticket_id filled"))?;

        info!(
            "Processing legacy worker output for ticket {}: outcome={:?}, target_stage={:?}",
            ticket_id, output.outcome, output.target_stage
        );

        // Validate that the ticket exists
        let ticket_exists = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id)
            .await?
            .is_some();

        if !ticket_exists {
            return Err(anyhow::anyhow!("Ticket '{}' not found", ticket_id));
        }

        // Convert to domain types for processing
        let ticket_id_domain = TicketId::new(ticket_id.clone())?;

        // Add worker comment
        self.add_worker_comment(&ticket_id_domain, &output.comment).await?;

        // Convert legacy format to new command structure
        match output.outcome {
            crate::workers::queue::WorkerOutcome::NextStage => {
                if let Some(target_stage_str) = &output.target_stage {
                    let target_stage = WorkerType::new(target_stage_str.clone())?;
                    let pipeline_update = output.pipeline_update.as_ref().map(|pipeline| {
                        pipeline.iter()
                            .filter_map(|s| WorkerType::new(s.clone()).ok())
                            .collect()
                    });

                    self.handlers.handle_advance_to_stage(
                        &ticket_id_domain,
                        &target_stage,
                        pipeline_update.as_ref()
                    ).await?;
                } else {
                    return Err(anyhow::anyhow!("next_stage outcome requires target_stage"));
                }
            }
            crate::workers::queue::WorkerOutcome::PrevStage => {
                if let Some(target_stage_str) = &output.target_stage {
                    let target_stage = WorkerType::new(target_stage_str.clone())?;
                    self.handlers.handle_return_to_stage(
                        &ticket_id_domain,
                        &target_stage,
                        &output.reason
                    ).await?;
                } else {
                    return Err(anyhow::anyhow!("prev_stage outcome requires target_stage"));
                }
            }
            crate::workers::queue::WorkerOutcome::CoordinatorAttention => {
                self.handlers.handle_coordinator_attention(
                    &ticket_id_domain,
                    &output.reason
                ).await?;
            }
        }

        Ok(())
    }

    async fn add_worker_comment(&self, ticket_id: &TicketId, comment: &str) -> Result<()> {
        crate::database::comments::Comment::create(
            &self.db,
            ticket_id.as_str(),
            Some("worker"),
            Some("system"),
            Some(1),
            comment,
        ).await?;
        
        info!("Added worker comment for ticket {}", ticket_id.as_str());
        Ok(())
    }
}