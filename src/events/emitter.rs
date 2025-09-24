/// Centralized event emission API that combines DB events and SSE broadcasting
use anyhow::Result;
use serde_json::Value;

use crate::{
    database::{events::Event, DbPool},
    events::EventPayload,
    sse::EventBroadcaster,
};

/// Central event emitter that handles both DB persistence and SSE broadcasting
pub struct EventEmitter<'a> {
    db: &'a DbPool,
    broadcaster: &'a EventBroadcaster,
}

impl<'a> EventEmitter<'a> {
    pub fn new(db: &'a DbPool, broadcaster: &'a EventBroadcaster) -> Self {
        Self { db, broadcaster }
    }

    /// Emit ticket created event with both DB and SSE
    pub async fn emit_ticket_created(
        &self,
        ticket_id: &str,
        project_id: &str,
        title: &str,
        current_stage: &str,
    ) -> Result<()> {
        // Create DB event
        Event::create(
            self.db,
            "ticket_created",
            Some(ticket_id),
            None,
            Some(current_stage),
            Some(&format!("Ticket '{}' created", title)),
        )
        .await?;

        // Broadcast SSE event
        let event =
            EventPayload::ticket_created_with_data(ticket_id, project_id, title, current_stage);
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted ticket_created event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit ticket updated event with both DB and SSE
    pub async fn emit_ticket_updated(
        &self,
        ticket_id: &str,
        project_id: &str,
        change_type: &str,
        stage: Option<&str>,
        reason: Option<&str>,
    ) -> Result<()> {
        // Create DB event
        Event::create(
            self.db,
            "ticket_updated",
            Some(ticket_id),
            None,
            stage,
            reason,
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::ticket_updated(ticket_id, project_id, change_type);
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted ticket_updated event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit ticket stage changed event with both DB and SSE
    pub async fn emit_ticket_stage_changed(
        &self,
        ticket_id: &str,
        project_id: &str,
        old_stage: &str,
        new_stage: &str,
        worker_id: Option<&str>,
    ) -> Result<()> {
        // Create DB event
        Event::create(
            self.db,
            "ticket_stage_updated",
            Some(ticket_id),
            worker_id,
            Some(new_stage),
            Some(&format!(
                "Stage changed from '{}' to '{}'",
                old_stage, new_stage
            )),
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::ticket_stage_changed(ticket_id, project_id, old_stage, new_stage);
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted ticket_stage_changed event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit ticket closed event with both DB and SSE
    pub async fn emit_ticket_closed(
        &self,
        ticket_id: &str,
        project_id: &str,
        resolution: &str,
    ) -> Result<()> {
        // Create DB event
        Event::create(
            self.db,
            "ticket_closed",
            Some(ticket_id),
            None,
            None,
            Some(&format!("Ticket closed with resolution: {}", resolution)),
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::ticket_closed(ticket_id, project_id);
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted ticket_closed event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit worker type created event (SSE only)
    pub async fn emit_worker_type_created(
        &self,
        project_id: &str,
        worker_type: &str,
        worker_type_data: &Value,
    ) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::system_message(
            "worker_types",
            "worker_type_created",
            Some(serde_json::json!({
                "worker_type": worker_type_data
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_type_created event for: {}/{}",
            project_id,
            worker_type
        );
        Ok(())
    }

    /// Emit worker type updated event (SSE only)
    pub async fn emit_worker_type_updated(
        &self,
        project_id: &str,
        worker_type: &str,
        worker_type_data: &Value,
    ) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::system_message(
            "worker_types",
            "worker_type_updated",
            Some(serde_json::json!({
                "worker_type": worker_type_data
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_type_updated event for: {}/{}",
            project_id,
            worker_type
        );
        Ok(())
    }

    /// Emit worker type deleted event (SSE only)
    pub async fn emit_worker_type_deleted(
        &self,
        project_id: &str,
        worker_type: &str,
    ) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::system_message(
            "worker_types",
            "worker_type_deleted",
            Some(serde_json::json!({
                "project_id": project_id,
                "worker_type": worker_type
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_type_deleted event for: {}/{}",
            project_id,
            worker_type
        );
        Ok(())
    }

    /// Emit project created event (SSE only)
    pub async fn emit_project_created(&self, project_data: &Value) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::system_message(
            "projects",
            "project_created",
            Some(serde_json::json!({
                "project": project_data
            })),
        );
        self.broadcaster.broadcast(event);

        if let Some(project_name) = project_data.get("repository_name").and_then(|v| v.as_str()) {
            tracing::debug!(
                "Successfully emitted project_created event for: {}",
                project_name
            );
        }
        Ok(())
    }

    /// Emit stage completed event with both DB and SSE
    pub async fn emit_stage_completed(
        &self,
        ticket_id: &str,
        stage: &str,
        worker_id: &str,
    ) -> Result<()> {
        // Create DB event
        Event::create_stage_completed(self.db, ticket_id, stage, worker_id).await?;

        // Broadcast SSE event
        let event = EventPayload::system_message(
            "stage_completed",
            &format!("Stage '{}' completed by worker {}", stage, worker_id),
            Some(serde_json::json!({
                "ticket_id": ticket_id,
                "stage": stage,
                "worker_id": worker_id
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted stage_completed event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit worker stopped event with both DB and SSE
    pub async fn emit_worker_stopped(&self, worker_id: &str, reason: &str) -> Result<()> {
        // Create DB event
        Event::create_worker_stopped(self.db, worker_id, reason).await?;

        // Broadcast SSE event
        let event = EventPayload::system_message(
            "worker_stopped",
            &format!("Worker {} stopped: {}", worker_id, reason),
            Some(serde_json::json!({
                "worker_id": worker_id,
                "reason": reason
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_stopped event for: {}",
            worker_id
        );
        Ok(())
    }

    /// Emit task assigned event with both DB and SSE
    pub async fn emit_task_assigned(&self, ticket_id: &str, queue_name: &str) -> Result<()> {
        // Create DB event
        Event::create_task_assigned(self.db, ticket_id, queue_name).await?;

        // Broadcast SSE event
        let event = EventPayload::system_message(
            "task_assigned",
            &format!("Task {} assigned to queue {}", ticket_id, queue_name),
            Some(serde_json::json!({
                "ticket_id": ticket_id,
                "queue_name": queue_name
            })),
        );
        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted task_assigned event for: {}",
            ticket_id
        );
        Ok(())
    }
}
