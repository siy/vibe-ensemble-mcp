use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::{
    database::{comments::CreateCommentRequest, tickets::Ticket},
    server::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
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

pub struct WorkerOutputProcessor;

impl WorkerOutputProcessor {
    /// Parse worker JSON output from a string
    pub fn parse_output(output: &str) -> Result<WorkerOutput> {
        // Try to find JSON in the output (workers might output other text too)
        let json_start = output.find('{');
        let json_end = output.rfind('}');

        match (json_start, json_end) {
            (Some(start), Some(end)) if start <= end => {
                let json_str = &output[start..=end];
                debug!("Parsing worker JSON: {}", json_str);
                let worker_output: WorkerOutput = serde_json::from_str(json_str)?;
                Ok(worker_output)
            }
            _ => {
                error!("No valid JSON found in worker output: {}", output);
                Err(anyhow::anyhow!("No valid JSON found in worker output"))
            }
        }
    }

    /// Process the parsed worker output and take appropriate actions
    pub async fn process_output(
        state: &AppState,
        ticket_id: &str,
        worker_id: &str,
        worker_type: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        info!(
            "Processing worker output for ticket {}: outcome={:?}, target_stage={:?}",
            ticket_id, output.outcome, output.target_stage
        );

        // First, add the worker's comment
        let comment_request = CreateCommentRequest {
            ticket_id: ticket_id.to_string(),
            worker_type: worker_type.to_string(),
            worker_id: worker_id.to_string(),
            stage_number: 1, // We'll keep stage numbers simple for now
            content: output.comment.clone(),
        };

        crate::database::comments::Comment::create(&state.db, comment_request).await?;
        info!("Added worker comment for ticket {}", ticket_id);

        // Process based on outcome
        match output.outcome {
            WorkerOutcome::NextStage => {
                Self::handle_next_stage(state, ticket_id, output).await?;
            }
            WorkerOutcome::PrevStage => {
                Self::handle_prev_stage(state, ticket_id, output).await?;
            }
            WorkerOutcome::CoordinatorAttention => {
                Self::handle_coordinator_attention(state, ticket_id, output).await?;
            }
        }

        Ok(())
    }

    async fn handle_next_stage(
        state: &AppState,
        ticket_id: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        let target_stage = output
            .target_stage
            .ok_or_else(|| anyhow::anyhow!("next_stage outcome requires target_stage"))?;

        info!(
            "Moving ticket {} to next stage: {}",
            ticket_id, target_stage
        );

        // Update pipeline if provided
        if let Some(new_pipeline) = &output.pipeline_update {
            info!(
                "Updating pipeline for ticket {} to: {:?}",
                ticket_id, new_pipeline
            );
            // Update the ticket's execution_plan (pipeline)
            let pipeline_json = serde_json::to_string(new_pipeline)?;
            sqlx::query(
                "UPDATE tickets SET execution_plan = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
            )
            .bind(&pipeline_json)
            .bind(ticket_id)
            .execute(&state.db)
            .await?;
        }

        // Move ticket to target stage
        Ticket::update_stage(&state.db, ticket_id, &target_stage).await?;

        info!(
            "Successfully moved ticket {} to stage {}",
            ticket_id, target_stage
        );
        Ok(())
    }

    async fn handle_prev_stage(
        state: &AppState,
        ticket_id: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        let target_stage = output
            .target_stage
            .ok_or_else(|| anyhow::anyhow!("prev_stage outcome requires target_stage"))?;

        warn!(
            "Moving ticket {} back to previous stage: {} (reason: {})",
            ticket_id, target_stage, output.reason
        );

        // Move ticket back to target stage
        Ticket::update_stage(&state.db, ticket_id, &target_stage).await?;

        // Create an event for tracking
        crate::database::events::Event::create_stage_completed(
            &state.db,
            ticket_id,
            &target_stage,
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
        state: &AppState,
        ticket_id: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        warn!(
            "Ticket {} requires coordinator attention: {}",
            ticket_id, output.reason
        );

        // Set ticket state to on_hold to signal coordinator intervention needed
        Ticket::update_state(&state.db, ticket_id, "on_hold").await?;

        // Create an event for coordinator attention
        crate::database::events::Event::create_stage_completed(
            &state.db,
            ticket_id,
            "coordinator_attention",
            "system",
        )
        .await?;

        // Add a special comment indicating coordinator attention is needed
        let coord_comment = CreateCommentRequest {
            ticket_id: ticket_id.to_string(),
            worker_type: "system".to_string(),
            worker_id: "system".to_string(),
            stage_number: 999, // Special stage for system messages
            content: format!("⚠️ COORDINATOR ATTENTION REQUIRED: {}", output.reason),
        };
        crate::database::comments::Comment::create(&state.db, coord_comment).await?;

        info!(
            "Set ticket {} to on_hold status for coordinator attention",
            ticket_id
        );
        Ok(())
    }
}
