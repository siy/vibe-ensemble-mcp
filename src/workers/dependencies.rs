use crate::{
    database::{tickets::Ticket, DbPool},
    sse::EventBroadcaster,
    workers::{domain::TicketId, queue::QueueManager},
};
use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Dependency management functionality for queue operations
pub struct DependencyManager;

impl Default for DependencyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyManager {
    /// Create a new DependencyManager instance
    pub fn new() -> Self {
        Self
    }
    /// Check and unblock dependent tickets when a ticket is completed
    pub async fn check_and_unblock_dependents(
        db: &DbPool,
        event_broadcaster: &EventBroadcaster,
        queue_manager: Arc<QueueManager>,
        ticket_id: &TicketId,
    ) -> Result<()> {
        info!(
            "Checking for dependent tickets to unblock after completing {}",
            ticket_id
        );

        // Find all tickets that depend on this ticket
        let dependent_tickets = sqlx::query_as::<_, Ticket>(
            r#"
            SELECT t.ticket_id, t.project_id, t.title, t.execution_plan, t.current_stage,
                   t.state, t.priority, t.processing_worker_id, t.created_at, t.updated_at,
                   t.closed_at, t.parent_ticket_id, t.dependency_status, t.created_by_worker_id,
                   t.ticket_type, t.rules_version, t.patterns_version, t.inherited_from_parent
            FROM tickets t
            INNER JOIN ticket_dependencies td ON t.ticket_id = td.dependent_ticket_id
            WHERE td.dependency_ticket_id = ?1 AND t.state = 'open' AND t.dependency_status = 'blocked'
            "#,
        )
        .bind(ticket_id.as_str())
        .fetch_all(db)
        .await?;

        for dependent_ticket in dependent_tickets {
            info!(
                "Checking if dependent ticket {} can be unblocked",
                dependent_ticket.ticket_id
            );

            // Check if all dependencies are satisfied
            let blocking_dependencies = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM ticket_dependencies td
                INNER JOIN tickets dep ON td.dependency_ticket_id = dep.ticket_id
                WHERE td.dependent_ticket_id = ?1
                AND dep.state != 'closed'
                "#,
            )
            .bind(&dependent_ticket.ticket_id)
            .fetch_one(db)
            .await?;

            if blocking_dependencies == 0 {
                // All dependencies satisfied, unblock the ticket
                info!(
                    "All dependencies satisfied for ticket {}, unblocking",
                    dependent_ticket.ticket_id
                );

                sqlx::query(
                    "UPDATE tickets SET dependency_status = 'ready', updated_at = datetime('now') WHERE ticket_id = ?1"
                )
                .bind(&dependent_ticket.ticket_id)
                .execute(db)
                .await?;

                // Resubmit to queue for processing
                let ticket_id = match TicketId::new(dependent_ticket.ticket_id.clone()) {
                    Ok(id) => id,
                    Err(e) => {
                        error!(
                            ticket_id = %dependent_ticket.ticket_id,
                            error = %e,
                            "Failed to create TicketId for resubmission"
                        );
                        continue; // Skip this ticket and continue with others
                    }
                };
                Self::resubmit_parent_ticket(
                    db,
                    event_broadcaster,
                    queue_manager.clone(),
                    &ticket_id,
                    &dependent_ticket.project_id,
                    &dependent_ticket.current_stage,
                )
                .await?;

                // Publish event
                let event = crate::events::EventPayload::ticket_updated(
                    &dependent_ticket.ticket_id,
                    &dependent_ticket.project_id,
                    "dependency_resolved",
                );
                event_broadcaster.broadcast(event);
            } else {
                info!(
                    "Ticket {} still has {} blocking dependencies",
                    dependent_ticket.ticket_id, blocking_dependencies
                );
            }
        }

        Ok(())
    }

    /// Resubmit a parent ticket for processing
    pub async fn resubmit_parent_ticket(
        db: &DbPool,
        event_broadcaster: &EventBroadcaster,
        queue_manager: Arc<QueueManager>,
        ticket_id: &TicketId,
        project_id: &str,
        current_stage: &str,
    ) -> Result<()> {
        info!(
            "Resubmitting parent ticket {} for processing in stage {}",
            ticket_id, current_stage
        );

        // Get fresh ticket data
        let _ticket = match Ticket::get_by_id(db, ticket_id.as_str()).await? {
            Some(ticket_with_comments) => ticket_with_comments.ticket,
            None => {
                warn!("Ticket {} not found when trying to resubmit", ticket_id);
                return Ok(());
            }
        };

        // Submit to queue for the current stage
        match queue_manager
            .submit_task(project_id, current_stage, ticket_id.as_str())
            .await
        {
            Ok(_) => {
                info!("Successfully resubmitted ticket {} to queue", ticket_id);

                // Publish event
                let event = crate::events::EventPayload::ticket_updated(
                    ticket_id.as_str(),
                    project_id,
                    "resubmitted",
                );
                event_broadcaster.broadcast(event);
            }
            Err(e) => {
                error!("Failed to resubmit ticket {} to queue: {}", ticket_id, e);
                return Err(e);
            }
        }

        Ok(())
    }
}
