/// Centralized event emission API that combines DB events and SSE broadcasting
use anyhow::Result;
use serde_json::Value;

use crate::{
    database::{events::Event, DbPool},
    events::{EventPayload, EventType},
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
            EventType::TicketCreated,
            Some(ticket_id),
            None,
            Some(current_stage),
            Some(&format!("Ticket '{}' created", title)),
        )
        .await?;

        // Broadcast SSE event
        let event =
            EventPayload::ticket_created_with_data(ticket_id, project_id, title, current_stage);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting ticket_created JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

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
            EventType::TicketUpdated,
            Some(ticket_id),
            None,
            stage,
            reason,
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::ticket_updated(ticket_id, project_id, change_type);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting ticket_updated JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

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
            EventType::TicketStageChanged,
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

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting ticket_stage_changed JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

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
            EventType::TicketClosed,
            Some(ticket_id),
            None,
            None,
            Some(&format!("Ticket closed with resolution: {}", resolution)),
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::ticket_closed(ticket_id, project_id);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting ticket_closed JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

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
        _worker_type_data: &Value,
    ) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::worker_type_created(project_id, worker_type);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_type_created JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
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
        _worker_type_data: &Value,
    ) -> Result<()> {
        // Broadcast SSE event
        let event = EventPayload::worker_type_updated(project_id, worker_type);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_type_updated JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
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
        let event = EventPayload::worker_type_deleted(project_id, worker_type);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_type_deleted JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
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
        let event = EventPayload::project_created(
            project_data
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown"),
        );

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting project_created JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
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
        let event = EventPayload::stage_completed(ticket_id, stage, worker_id);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting stage_completed JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted stage_completed event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit worker stopped event with both DB and SSE
    pub async fn emit_worker_stopped(&self, worker_id: &str, worker_type: &str, project_id: &str, reason: &str) -> Result<()> {
        // Create DB event
        Event::create_worker_stopped(self.db, worker_id, reason).await?;

        // Broadcast SSE event
        let event = EventPayload::worker_stopped(worker_id, worker_type, project_id, reason);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_stopped JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
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
        let event = EventPayload::task_assigned(ticket_id, queue_name);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting task_assigned JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted task_assigned event for: {}",
            ticket_id
        );
        Ok(())
    }

    /// Emit worker started event with both DB and SSE
    pub async fn emit_worker_started(
        &self,
        worker_id: &str,
        worker_type: &str,
        project_id: &str,
    ) -> Result<()> {
        // Create DB event
        let message = format!(
            "Worker {} started for project {}",
            worker_id, project_id
        );
        Event::create(
            self.db,
            EventType::WorkerStarted,
            None,
            Some(worker_id),
            Some(worker_type),
            Some(message.as_str()),
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::worker_started(worker_id, worker_type, project_id);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_started JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_started event for: {}",
            worker_id
        );
        Ok(())
    }

    /// Emit worker completed event with both DB and SSE
    pub async fn emit_worker_completed(
        &self,
        worker_id: &str,
        worker_type: &str,
        project_id: &str,
    ) -> Result<()> {
        // Create DB event
        let message = format!(
            "Worker {} completed for project {}",
            worker_id, project_id
        );
        Event::create(
            self.db,
            EventType::WorkerCompleted,
            None,
            Some(worker_id),
            Some(worker_type),
            Some(message.as_str()),
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::worker_completed(worker_id, worker_type, project_id);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_completed JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_completed event for: {}",
            worker_id
        );
        Ok(())
    }

    /// Emit worker failed event with both DB and SSE
    pub async fn emit_worker_failed(
        &self,
        worker_id: &str,
        worker_type: &str,
        project_id: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        // Create DB event
        let failure_reason: Option<String>;
        let reason_ref = match reason {
            Some(r) => Some(r),
            None => {
                failure_reason = Some(format!(
                    "Worker {} failed for project {}",
                    worker_id, project_id
                ));
                failure_reason.as_deref()
            }
        };
        Event::create(
            self.db,
            EventType::WorkerFailed,
            None,
            Some(worker_id),
            Some(worker_type),
            reason_ref,
        )
        .await?;

        // Broadcast SSE event
        let event = EventPayload::worker_failed(worker_id, worker_type, project_id);

        // Log the complete JSON-RPC message at debug level
        let jsonrpc_message = event.to_jsonrpc_notification();
        tracing::debug!(
            "Broadcasting worker_failed JSON-RPC: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );

        self.broadcaster.broadcast(event);

        tracing::debug!(
            "Successfully emitted worker_failed event for: {}",
            worker_id
        );
        Ok(())
    }
}
