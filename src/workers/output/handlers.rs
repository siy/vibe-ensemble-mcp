use crate::workers::domain::*;
use crate::database::DbPool;
use anyhow::Result;
use tracing::{debug, info, warn};

pub struct OutputHandlers {
    db: DbPool,
}

impl OutputHandlers {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    pub async fn handle_advance_to_stage(
        &self,
        ticket_id: &TicketId,
        target_stage: &WorkerType,
        pipeline_update: Option<&Vec<WorkerType>>,
    ) -> Result<()> {
        // Update pipeline FIRST if provided - this allows worker types to be created during planning
        if let Some(new_pipeline) = pipeline_update {
            self.update_pipeline(ticket_id, new_pipeline).await?;
        }

        // Validate that the target worker type exists in the project
        self.validate_worker_type_exists(ticket_id, target_stage).await?;

        info!(
            "Moving ticket {} to next stage: {}",
            ticket_id.as_str(), 
            target_stage.as_str()
        );

        self.transition_ticket_stage(ticket_id, target_stage).await
    }

    pub async fn handle_return_to_stage(
        &self,
        ticket_id: &TicketId,
        target_stage: &WorkerType,
        reason: &str,
    ) -> Result<()> {
        // Validate target stage
        self.validate_worker_type_exists(ticket_id, target_stage).await?;

        warn!(
            "Moving ticket {} back to previous stage: {} (reason: {})",
            ticket_id.as_str(),
            target_stage.as_str(),
            reason
        );

        self.transition_ticket_stage(ticket_id, target_stage).await
    }

    pub async fn handle_coordinator_attention(
        &self,
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
            "on_hold"
        ).await?;

        // Create coordinator attention event
        crate::database::events::Event::create_stage_completed(
            &self.db,
            ticket_id.as_str(),
            "coordinator_attention",
            "system",
        ).await?;

        // Add special comment
        crate::database::comments::Comment::create(
            &self.db,
            ticket_id.as_str(),
            Some("system"),
            Some("system"),
            Some(999), // Special stage for system messages
            &format!("⚠️ COORDINATOR ATTENTION REQUIRED: {}", reason),
        ).await?;

        info!(
            "Set ticket {} to on_hold status for coordinator attention",
            ticket_id.as_str()
        );

        Ok(())
    }

    // Private helper methods
    async fn update_pipeline(
        &self,
        ticket_id: &TicketId,
        new_pipeline: &[WorkerType],
    ) -> Result<()> {
        info!(
            "Updating pipeline for ticket {} to: {:?}",
            ticket_id.as_str(), 
            new_pipeline
        );

        // Get ticket to find project_id for pipeline validation
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id.as_str()))?;

        let _project_id = &ticket_with_comments.ticket.project_id;

        // Convert WorkerType to strings for database
        let pipeline_strings: Vec<String> = new_pipeline.iter()
            .map(|wt| wt.as_str().to_string())
            .collect();

        // Update the execution_plan field in the database  
        let pipeline_json = serde_json::to_string(&pipeline_strings)
            .map_err(|e| anyhow::anyhow!("Failed to serialize pipeline: {}", e))?;
        
        sqlx::query(
            "UPDATE tickets SET execution_plan = ?1, updated_at = datetime('now') WHERE ticket_id = ?2"
        )
        .bind(pipeline_json)
        .bind(ticket_id.as_str())
        .execute(&self.db)
        .await?;

        info!("Successfully updated pipeline for ticket {}", ticket_id.as_str());
        Ok(())
    }

    async fn validate_worker_type_exists(
        &self,
        ticket_id: &TicketId,
        worker_type: &WorkerType,
    ) -> Result<()> {
        // Get ticket to find project_id
        let ticket_with_comments = crate::database::tickets::Ticket::get_by_id(&self.db, ticket_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id.as_str()))?;

        // Check if worker type exists in the project
        let worker_type_exists = crate::database::worker_types::WorkerType::get_by_type(
            &self.db,
            &ticket_with_comments.ticket.project_id,
            worker_type.as_str(),
        )
        .await?
        .is_some();

        if !worker_type_exists {
            return Err(anyhow::anyhow!(
                "Worker type '{}' does not exist in project '{}'",
                worker_type.as_str(),
                ticket_with_comments.ticket.project_id
            ));
        }

        Ok(())
    }

    async fn transition_ticket_stage(
        &self,
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
        ).await?;

        // Create completion event
        crate::database::events::Event::create_stage_completed(
            &self.db,
            ticket_id.as_str(),
            target_stage.as_str(),
            "system",
        ).await?;

        info!(
            "Successfully moved ticket {} to stage {}",
            ticket_id.as_str(),
            target_stage.as_str()
        );

        Ok(())
    }

    async fn release_ticket_if_claimed(&self, ticket_id: &TicketId) -> Result<()> {
        debug!("Releasing ticket {} if claimed", ticket_id.as_str());

        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL
            "#,
        )
        .bind(ticket_id.as_str())
        .execute(&self.db)
        .await?;

        if result.rows_affected() > 0 {
            info!("Released claimed ticket {} for stage transition", ticket_id.as_str());
        } else {
            debug!("Ticket {} was not claimed, no release needed", ticket_id.as_str());
        }

        Ok(())
    }
}