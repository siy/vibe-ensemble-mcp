/// Centralized pipeline and worker type validation
use anyhow::Result;
use tracing::info;

use crate::database::{tickets::Ticket, worker_types::WorkerType, DbPool};

/// Validation helper for pipeline stages and worker types
pub struct PipelineValidator;

impl PipelineValidator {
    /// Validate that a single worker type exists for a project
    /// Returns a consistent error message for missing worker types
    pub async fn validate_worker_type_exists(
        db: &DbPool,
        project_id: &str,
        worker_type: &str,
    ) -> Result<()> {
        let worker_type_exists = WorkerType::get_by_type(db, project_id, worker_type)
            .await?
            .is_some();

        if !worker_type_exists {
            return Err(anyhow::anyhow!(
                "Worker type '{}' does not exist for project '{}'. Worker types must be created before use.",
                worker_type,
                project_id
            ));
        }

        Ok(())
    }

    /// Validate that a worker type exists for a ticket (looks up project_id)
    pub async fn validate_worker_type_exists_for_ticket(
        db: &DbPool,
        ticket_id: &str,
        worker_type: &str,
    ) -> Result<()> {
        // Get ticket to find project_id
        let ticket = Ticket::get_by_id(db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        Self::validate_worker_type_exists(db, &ticket.ticket.project_id, worker_type).await
    }

    /// Validate that all stages in a pipeline exist as worker types
    /// Used for both ticket creation and pipeline updates
    pub async fn validate_pipeline_stages(
        db: &DbPool,
        project_id: &str,
        pipeline_stages: &[String],
        context: &str, // "pipeline update", "ticket creation", etc.
    ) -> Result<()> {
        for stage in pipeline_stages {
            if let Err(e) = Self::validate_worker_type_exists(db, project_id, stage).await {
                return Err(anyhow::anyhow!("{} validation failed: {}", context, e));
            }
        }

        info!(
            "{} validation passed: all {} stages exist as worker types in project '{}'",
            context,
            pipeline_stages.len(),
            project_id
        );

        Ok(())
    }

    /// Validate a ticket's initial stage during creation
    pub async fn validate_initial_stage(
        db: &DbPool,
        project_id: &str,
        initial_stage: &str,
    ) -> Result<()> {
        Self::validate_worker_type_exists(db, project_id, initial_stage)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Worker type '{}' does not exist for project '{}'. Cannot use as initial stage. Coordinator must create this worker type first.",
                    initial_stage,
                    project_id
                )
            })
    }

    /// Validate a target stage for ticket resumption
    pub async fn validate_resume_stage(
        db: &DbPool,
        project_id: &str,
        target_stage: &str,
    ) -> Result<()> {
        // Allow "planning" stage without validation (built-in stage)
        if target_stage == "planning" {
            return Ok(());
        }

        Self::validate_worker_type_exists(db, project_id, target_stage)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Worker type '{}' does not exist for project '{}'. Cannot resume ticket with this stage.",
                    target_stage,
                    project_id
                )
            })
    }
}
