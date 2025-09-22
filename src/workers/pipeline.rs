use crate::{
    database::{tickets::Ticket, DbPool},
    validation::PipelineValidator,
};
use anyhow::Result;
use tracing::info;

/// Pipeline management functionality for queue operations
pub struct PipelineManager;

impl PipelineManager {
    /// Create a new PipelineManager instance
    pub fn new() -> Self {
        Self
    }
    /// Get the current stage index in the execution plan
    pub fn get_current_stage_index(ticket: &Ticket) -> Result<usize> {
        let pipeline: Vec<String> = serde_json::from_str(&ticket.execution_plan)?;

        let stage_index = pipeline
            .iter()
            .position(|stage| stage == &ticket.current_stage)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Current stage '{}' not found in pipeline: {:?}",
                    ticket.current_stage,
                    pipeline
                )
            })?;

        Ok(stage_index)
    }

    /// Validate that pipeline update preserves past stages
    pub fn validate_pipeline_preserves_past_stages(
        current_pipeline: &[String],
        new_pipeline: &[String],
        current_stage_index: usize,
    ) -> Result<()> {
        // Check that past stages are preserved (up to current_stage_index)
        for i in 0..=current_stage_index {
            if i >= new_pipeline.len() {
                return Err(anyhow::anyhow!(
                    "New pipeline is shorter than current stage index. Current index: {}, new length: {}",
                    current_stage_index,
                    new_pipeline.len()
                ));
            }

            if current_pipeline.get(i) != new_pipeline.get(i) {
                return Err(anyhow::anyhow!(
                    "Pipeline update would change past stage at index {}. Current: {:?}, New: {:?}",
                    i,
                    current_pipeline.get(i),
                    new_pipeline.get(i)
                ));
            }
        }

        Ok(())
    }

    /// Update pipeline and stage in database with validation
    pub async fn update_pipeline_and_stage(
        db: &DbPool,
        ticket: &Ticket,
        new_pipeline: Vec<String>,
        target_stage: &str,
    ) -> Result<()> {
        let current_stage_index = Self::get_current_stage_index(ticket)?;
        let current_pipeline: Vec<String> = serde_json::from_str(&ticket.execution_plan)?;

        // Validate pipeline preserves past stages
        Self::validate_pipeline_preserves_past_stages(
            &current_pipeline,
            &new_pipeline,
            current_stage_index,
        )?;

        // Validate pipeline stages exist as worker types
        PipelineValidator::validate_pipeline_stages(
            db,
            &ticket.project_id,
            &new_pipeline,
            "pipeline update",
        )
        .await?;

        // Update pipeline in database
        let new_execution_plan = serde_json::to_string(&new_pipeline)?;
        sqlx::query("UPDATE tickets SET execution_plan = ?1 WHERE ticket_id = ?2")
            .bind(new_execution_plan)
            .bind(&ticket.ticket_id)
            .execute(db)
            .await?;

        info!(
            "Updated pipeline for ticket {}: {:?} -> {:?}",
            ticket.ticket_id, current_pipeline, new_pipeline
        );

        // Update current stage
        sqlx::query(
            "UPDATE tickets SET current_stage = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
        )
        .bind(target_stage)
        .bind(&ticket.ticket_id)
        .execute(db)
        .await?;

        info!(
            "Updated current stage for ticket {} from '{}' to '{}'",
            ticket.ticket_id, ticket.current_stage, target_stage
        );

        Ok(())
    }
}
