use anyhow::Result;
use sqlx::Row;
use tracing::{info, warn};

use super::{tickets::TicketState, DbPool};

/// Recovery statistics for ticket processing
#[derive(Debug, Default)]
pub struct RecoveryStats {
    pub tickets_recovered: usize,
    pub claimed_tickets_released: usize,
    pub on_hold_tickets_recovered: usize,
}

/// Represents a ticket that needs recovery
#[derive(Debug)]
pub struct UnprocessedTicket {
    pub ticket_id: String,
    pub project_id: String,
    pub current_stage: String,
    pub state: String,
    pub processing_worker_id: Option<String>,
    pub minutes_since_update: f64,
}

impl UnprocessedTicket {
    /// Get ticket state as typed enum
    pub fn get_state(&self) -> Result<TicketState> {
        self.state.parse()
    }

    /// Check if ticket is open
    pub fn is_open(&self) -> bool {
        matches!(self.get_state().ok(), Some(TicketState::Open))
    }

    /// Check if ticket is on hold
    pub fn is_on_hold(&self) -> bool {
        matches!(self.get_state().ok(), Some(TicketState::OnHold))
    }
}

/// Recovery operations for tickets and workers
pub struct TicketRecovery;

impl TicketRecovery {
    /// Find all tickets that need recovery (unprocessed, stalled, or on-hold)
    pub async fn find_unprocessed_tickets(db: &DbPool) -> Result<Vec<UnprocessedTicket>> {
        let rows = sqlx::query(
            r#"
            SELECT ticket_id, project_id, current_stage, state, processing_worker_id,
                   datetime('now') AS current_time, updated_at,
                   (julianday('now') - julianday(updated_at)) * 24 * 60 AS minutes_since_update
            FROM tickets
            WHERE dependency_status = 'ready'
              AND (
                -- Case 1: Open tickets not being processed
                (state = 'open' AND processing_worker_id IS NULL)
                OR
                -- Case 2: Open tickets claimed but stalled (no update for >5 minutes)
                (state = 'open' AND processing_worker_id IS NOT NULL
                 AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 5)
                OR
                -- Case 3: On-hold tickets that may be recoverable
                (state = 'on_hold')
              )
            ORDER BY project_id, current_stage, priority DESC, created_at ASC
            "#,
        )
        .fetch_all(db)
        .await?;

        let tickets = rows
            .into_iter()
            .map(|row| UnprocessedTicket {
                ticket_id: row.get("ticket_id"),
                project_id: row.get("project_id"),
                current_stage: row.get("current_stage"),
                state: row.get("state"),
                processing_worker_id: row.get("processing_worker_id"),
                minutes_since_update: row.get("minutes_since_update"),
            })
            .collect();

        Ok(tickets)
    }

    /// Release a stalled claim on a ticket
    pub async fn release_stalled_claim(db: &DbPool, ticket_id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tickets
            SET processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL
            "#,
        )
        .bind(ticket_id)
        .execute(db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Recover an on-hold ticket back to open state
    pub async fn recover_on_hold_ticket(db: &DbPool, ticket_id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tickets
            SET state = 'open', processing_worker_id = NULL, updated_at = datetime('now')
            WHERE ticket_id = ?1 AND state = 'on_hold'
            "#,
        )
        .bind(ticket_id)
        .execute(db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Process recovery for all unprocessed tickets
    pub async fn process_recovery(db: &DbPool) -> Result<RecoveryStats> {
        info!("Starting enhanced ticket recovery system...");

        let unprocessed_tickets = Self::find_unprocessed_tickets(db).await?;

        if unprocessed_tickets.is_empty() {
            info!("No unprocessed tickets found for recovery");
            return Ok(RecoveryStats::default());
        }

        let mut stats = RecoveryStats::default();

        for ticket in unprocessed_tickets {
            // Handle different recovery scenarios
            if ticket.is_open() && ticket.processing_worker_id.is_some() {
                // Stalled claimed ticket - release claim first
                let worker_id = ticket
                    .processing_worker_id
                    .as_ref()
                    .unwrap_or(&"unknown".to_string())
                    .clone();
                warn!(
                    "Releasing stalled claim for ticket {} (worker: {}, stalled for {:.1} minutes)",
                    ticket.ticket_id, worker_id, ticket.minutes_since_update
                );

                if Self::release_stalled_claim(db, &ticket.ticket_id).await? {
                    stats.claimed_tickets_released += 1;
                    info!("Released stalled claim for ticket {}", ticket.ticket_id);
                }
            } else if ticket.is_on_hold() {
                // On-hold ticket - attempt to bring back to open state
                info!(
                    "Recovering on-hold ticket {} (on hold for {:.1} minutes)",
                    ticket.ticket_id, ticket.minutes_since_update
                );

                if Self::recover_on_hold_ticket(db, &ticket.ticket_id).await? {
                    stats.on_hold_tickets_recovered += 1;
                    info!(
                        "Recovered on-hold ticket {} back to open state",
                        ticket.ticket_id
                    );
                }
            }

            // Count all tickets that were processed for recovery
            stats.tickets_recovered += 1;
        }

        info!(
            "Enhanced ticket recovery completed: {} tickets recovered, {} stalled claims released, {} on-hold tickets recovered",
            stats.tickets_recovered, stats.claimed_tickets_released, stats.on_hold_tickets_recovered
        );

        Ok(stats)
    }

    /// Get tickets ready for resubmission to queues
    pub async fn get_tickets_for_resubmission(
        db: &DbPool,
    ) -> Result<Vec<(String, String, String)>> {
        let rows = sqlx::query(
            r#"
            SELECT ticket_id, project_id, current_stage
            FROM tickets
            WHERE state = 'open'
              AND processing_worker_id IS NULL
              AND dependency_status = 'ready'
            ORDER BY project_id, current_stage, priority DESC, created_at ASC
            "#,
        )
        .fetch_all(db)
        .await?;

        let tickets = rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("ticket_id"),
                    row.get::<String, _>("project_id"),
                    row.get::<String, _>("current_stage"),
                )
            })
            .collect();

        Ok(tickets)
    }
}
