use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    database::{
        comments::CreateCommentRequest, tickets::Ticket, worker_types::WorkerType, workers::Worker,
    },
    server::AppState,
    workers::{process::ProcessManager, types::SpawnWorkerRequest},
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

pub struct WorkerOutputProcessor;

impl WorkerOutputProcessor {
    /// Automatically spawn a worker for a given stage if none are active
    pub async fn auto_spawn_worker_for_stage(
        state: &AppState,
        project_id: &str,
        stage: &str,
    ) -> Result<()> {
        // Check if there's already an active worker for this stage
        let existing_workers = sqlx::query_as::<_, Worker>(
            r#"
            SELECT worker_id, project_id, worker_type, status, pid, queue_name, started_at, last_activity
            FROM workers 
            WHERE project_id = ?1 AND worker_type = ?2 AND status IN ('spawning', 'active', 'idle')
        "#,
        )
        .bind(project_id)
        .bind(stage)
        .fetch_all(&state.db)
        .await?;

        // Check if any existing workers are actually running
        for worker in &existing_workers {
            if let Some(pid) = worker.pid {
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);

                if is_running {
                    info!(
                        "Worker {} already active for stage {} in project {}",
                        worker.worker_id, stage, project_id
                    );
                    return Ok(());
                }
            }
        }

        // Generate unique worker ID
        let worker_id = format!("{}-{}", stage, &Uuid::new_v4().to_string()[..8]);

        let spawn_request = SpawnWorkerRequest {
            worker_id: worker_id.clone(),
            project_id: project_id.to_string(),
            worker_type: stage.to_string(),
            queue_name: format!("{}-queue", stage),
        };

        match ProcessManager::spawn_worker(state, spawn_request).await {
            Ok(_worker_process) => {
                info!(
                    "Auto-spawned worker {} for stage {} in project {}",
                    worker_id, stage, project_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to auto-spawn worker for stage {} in project {}: {}",
                    stage, project_id, e
                );
            }
        }

        Ok(())
    }

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

    /// Validate that a target stage has a corresponding worker type registration
    async fn validate_target_stage(
        state: &AppState,
        ticket_id: &str,
        target_stage: &str,
    ) -> Result<String> {
        // Get ticket to find project_id
        let ticket_with_comments = Ticket::get_by_id(&state.db, ticket_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

        // Check if worker type exists for this project and stage
        let worker_type_exists = WorkerType::get_by_type(
            &state.db,
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

        Ok(ticket_with_comments.ticket.project_id)
    }

    /// Process the parsed worker output and take appropriate actions
    pub async fn process_output(
        state: &AppState,
        external_ticket_id: &str, // Used as fallback when worker output doesn't include ticket_id
        worker_id: &str,
        worker_type: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        // Use ticket ID from worker output if provided, otherwise fallback to external parameter
        let ticket_id = output
            .ticket_id
            .clone()
            .unwrap_or_else(|| external_ticket_id.to_string());

        info!(
            "Processing worker output for ticket {}: outcome={:?}, target_stage={:?}",
            ticket_id, output.outcome, output.target_stage
        );

        // First, validate that the ticket exists
        let ticket_exists = Ticket::get_by_id(&state.db, &ticket_id).await?.is_some();

        if !ticket_exists {
            return Err(anyhow::anyhow!("Ticket '{}' not found", ticket_id));
        }

        // Now add the worker's comment
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
                Self::handle_next_stage(state, &ticket_id, worker_id, output).await?;
            }
            WorkerOutcome::PrevStage => {
                Self::handle_prev_stage(state, &ticket_id, output).await?;
            }
            WorkerOutcome::CoordinatorAttention => {
                Self::handle_coordinator_attention(state, &ticket_id, output).await?;
            }
        }

        Ok(())
    }

    async fn handle_next_stage(
        state: &AppState,
        ticket_id: &str,
        worker_id: &str,
        output: WorkerOutput,
    ) -> Result<()> {
        let target_stage = output
            .target_stage
            .ok_or_else(|| anyhow::anyhow!("next_stage outcome requires target_stage"))?;

        // Update pipeline FIRST if provided - this allows worker types to be created during planning
        if let Some(new_pipeline) = &output.pipeline_update {
            info!(
                "Updating pipeline for ticket {} to: {:?}",
                ticket_id, new_pipeline
            );

            // Get ticket to find project_id for pipeline validation
            let ticket_with_comments = Ticket::get_by_id(&state.db, ticket_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Ticket '{}' not found", ticket_id))?;

            // Validate all stages in the new pipeline have registered worker types
            for stage in new_pipeline {
                let worker_type_exists = WorkerType::get_by_type(
                    &state.db,
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
            .execute(&state.db)
            .await?;
        }

        // Now validate that the target stage has a registered worker type
        Self::validate_target_stage(state, ticket_id, &target_stage).await?;

        info!(
            "Moving ticket {} to next stage: {}",
            ticket_id, target_stage
        );

        // Release ticket from current worker (if claimed)
        Self::release_ticket_if_claimed(state, ticket_id).await?;

        // Move ticket to target stage
        Ticket::update_stage(&state.db, ticket_id, &target_stage).await?;

        // Create an event for successful stage completion
        crate::database::events::Event::create_stage_completed(
            &state.db,
            ticket_id,
            &target_stage,
            worker_id,
        )
        .await?;

        // TODO: Auto-spawn worker for the new stage (disabled due to Send constraint)
        // This will be handled by coordinator monitoring instead
        info!(
            "Successfully moved ticket {} to stage {} - coordinator should spawn worker for this stage",
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

        // Validate that the target stage has a registered worker type
        Self::validate_target_stage(state, ticket_id, &target_stage).await?;

        warn!(
            "Moving ticket {} back to previous stage: {} (reason: {})",
            ticket_id, target_stage, output.reason
        );

        // Release ticket from current worker (if claimed)
        Self::release_ticket_if_claimed(state, ticket_id).await?;

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

    /// Release a ticket if it's currently claimed by any worker
    async fn release_ticket_if_claimed(state: &AppState, ticket_id: &str) -> Result<()> {
        debug!("Releasing ticket {} if claimed", ticket_id);

        let result = sqlx::query(
            r#"
            UPDATE tickets 
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL
            "#,
        )
        .bind(ticket_id)
        .execute(&state.db)
        .await?;

        if result.rows_affected() > 0 {
            info!("Released claimed ticket {} for stage transition", ticket_id);
        } else {
            debug!("Ticket {} was not claimed, no release needed", ticket_id);
        }

        Ok(())
    }
}
