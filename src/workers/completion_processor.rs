use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{
    claims::ClaimManager, dependencies::DependencyManager,
    transitions::TicketTransitionManager,
};
use crate::{
    database::DbPool,
    events::EventPayload,
    sse::EventBroadcaster,
    workers::domain::{WorkerCommand, WorkerCompletionEvent},
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

/// Processes worker completion events and handles ticket state transitions
pub struct CompletionProcessor {
    db: DbPool,
    event_broadcaster: EventBroadcaster,
    transition_manager: TicketTransitionManager,
    queue_manager: std::sync::Arc<super::queue::QueueManager>,
}

impl CompletionProcessor {
    pub fn new(
        db: DbPool,
        event_broadcaster: EventBroadcaster,
        queue_manager: std::sync::Arc<super::queue::QueueManager>,
    ) -> Self {
        let transition_manager = TicketTransitionManager::new(db.clone());

        Self {
            db,
            event_broadcaster,
            transition_manager,
            queue_manager,
        }
    }

    /// Start the completion processor loop
    pub async fn run(
        self: Arc<Self>,
        mut completion_receiver: mpsc::Receiver<WorkerCompletionEvent>,
        queue_submit_sender: mpsc::Sender<(String, String, String)>, // (project_id, stage, ticket_id)
    ) -> Result<()> {
        info!("Starting WorkerCompletionEvent processor with auto-enqueue");

        while let Some(completion_event) = completion_receiver.recv().await {
            if let Err(e) = self
                .process_completion(completion_event, &queue_submit_sender)
                .await
            {
                error!(error = %e, "Failed to process completion event");
            }
        }

        warn!("Completion processor loop ended");
        Ok(())
    }

    /// Process a single worker completion event
    async fn process_completion(
        &self,
        event: WorkerCompletionEvent,
        queue_submit_sender: &mpsc::Sender<(String, String, String)>,
    ) -> Result<()> {
        let ticket_id = event.ticket_id.as_str();

        debug!(
            ticket_id = %ticket_id,
            "Processing worker completion event"
        );

        info!(
            ticket_id = %ticket_id,
            command = ?event.command,
            "Processing completion for ticket"
        );

        // Process the completion based on command
        let _next_stage = match &event.command {
            WorkerCommand::AdvanceToStage { target_stage, pipeline_update: _ } => {
                self.handle_next_stage(ticket_id, target_stage.as_str(), &event.comment, queue_submit_sender)
                    .await?
            }
            WorkerCommand::ReturnToStage { target_stage, reason: _ } => {
                self.handle_prev_stage(ticket_id, target_stage.as_str(), &event.comment, queue_submit_sender)
                    .await?
            }
            WorkerCommand::RequestCoordinatorAttention { reason: _ } => {
                self.handle_coordinator_attention(ticket_id, &event.comment).await?;
                None
            }
        };

        // Release the claim
        if let Err(e) = ClaimManager::release_ticket_claim(&self.db, &self.event_broadcaster, ticket_id).await {
            error!(
                ticket_id = %ticket_id,
                error = %e,
                "Failed to release claim after completion"
            );
        }

        // Emit completion event with proper project info
        if let Ok(Some(ticket_info)) = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id).await {
            let worker_id = format!("{}:{}:{}", ticket_info.ticket.project_id, ticket_info.ticket.current_stage, ticket_id);
            let event_payload = EventPayload::worker_completed(
                &worker_id,
                &ticket_info.ticket.current_stage,
                &ticket_info.ticket.project_id
            );
            self.event_broadcaster.broadcast(event_payload);
        } else {
            warn!(ticket_id = %ticket_id, "Could not fetch ticket info for worker completion event");
        }

        // Check and process dependency cascades
        if let Err(e) = DependencyManager::check_and_unblock_dependents(
            &self.db,
            &self.event_broadcaster,
            self.queue_manager.clone(),
            &crate::workers::domain::TicketId::new(ticket_id.to_string()).unwrap(),
        ).await {
            error!(
                ticket_id = %ticket_id,
                error = %e,
                "Failed to process dependency cascade"
            );
        }

        Ok(())
    }

    /// Handle transition to next stage
    async fn handle_next_stage(
        &self,
        ticket_id: &str,
        target_stage: &str,
        comment: &str,
        queue_submit_sender: &mpsc::Sender<(String, String, String)>,
    ) -> Result<Option<String>> {
        // Pipeline updates would be handled here if pipeline_update was provided in WorkerCommand
        // This is a placeholder for future enhancement

        // Use the provided target stage
        let next_stage = target_stage.to_string();

        // Transition ticket to next stage
        self.transition_manager
            .transition_to_stage(ticket_id, &next_stage, comment)
            .await?;

        // Submit to queue if not completed
        if next_stage != "completed" {
            let (project_id, _) = self.transition_manager.get_ticket_info(ticket_id).await?;

            if let Err(e) = queue_submit_sender.send((project_id, next_stage.clone(), ticket_id.to_string())).await {
                error!(
                    ticket_id = %ticket_id,
                    next_stage = %next_stage,
                    error = %e,
                    "Failed to submit ticket to next stage queue"
                );
            }
        }

        Ok(Some(next_stage))
    }

    /// Handle transition to previous stage
    async fn handle_prev_stage(
        &self,
        ticket_id: &str,
        target_stage: &str,
        comment: &str,
        queue_submit_sender: &mpsc::Sender<(String, String, String)>,
    ) -> Result<Option<String>> {
        // Use the provided target stage
        let prev_stage = target_stage.to_string();

        // Transition ticket to previous stage
        self.transition_manager
            .transition_to_stage(ticket_id, &prev_stage, comment)
            .await?;

        // Submit to queue
        let (project_id, _) = self.transition_manager.get_ticket_info(ticket_id).await?;

        if let Err(e) = queue_submit_sender.send((project_id, prev_stage.clone(), ticket_id.to_string())).await {
            error!(
                ticket_id = %ticket_id,
                prev_stage = %prev_stage,
                error = %e,
                "Failed to submit ticket to previous stage queue"
            );
        }

        Ok(Some(prev_stage))
    }

    /// Handle coordinator attention
    async fn handle_coordinator_attention(
        &self,
        ticket_id: &str,
        comment: &str,
    ) -> Result<()> {
        // Place ticket on hold for coordinator attention
        self.transition_manager
            .place_on_hold(ticket_id, comment)
            .await?;

        info!(
            ticket_id = %ticket_id,
            comment = %comment,
            "Ticket placed on hold for coordinator attention"
        );

        Ok(())
    }

}