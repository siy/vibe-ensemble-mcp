use crate::{database::DbPool, workers::domain::TicketId};
use anyhow::Result;
use tracing::{error, info, warn};

/// Result type for ticket claim operations
#[derive(Debug)]
pub enum ClaimResult {
    /// Ticket successfully claimed
    Success,
    /// Ticket already claimed by another worker
    AlreadyClaimed(String),
    /// Ticket cannot be claimed due to its current state
    NotClaimable {
        state: String,
        dependency_status: String,
    },
}

/// Claim management functionality for queue operations
pub struct ClaimManager;

impl Default for ClaimManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaimManager {
    /// Create a new ClaimManager instance
    pub fn new() -> Self {
        Self
    }
    /// Claim a ticket for processing with detailed result information
    pub async fn claim_for_processing(
        db: &DbPool,
        ticket_id: &TicketId,
        worker_id: &str,
    ) -> Result<ClaimResult> {
        // Use a transaction for atomic claim verification
        let mut tx = db.begin().await?;

        // Attempt atomic UPDATE
        let result = sqlx::query(
            r#"
            UPDATE tickets
            SET processing_worker_id = ?1, updated_at = datetime('now')
            WHERE ticket_id = ?2
              AND processing_worker_id IS NULL
              AND state = 'open'
              AND dependency_status = 'ready'
        "#,
        )
        .bind(worker_id)
        .bind(ticket_id.as_str())
        .execute(&mut *tx)
        .await?;

        let rows_affected = result.rows_affected();

        if rows_affected == 0 {
            // Fetch current state to provide detailed error
            let ticket_state = sqlx::query_as::<_, (String, Option<String>, String)>(
                "SELECT state, processing_worker_id, dependency_status FROM tickets WHERE ticket_id = ?1"
            )
            .bind(ticket_id.as_str())
            .fetch_optional(&mut *tx)
            .await?;

            tx.rollback().await?;

            return match ticket_state {
                Some((state, Some(current_worker), _)) if state == "open" => {
                    warn!(
                        "Ticket {} already claimed by worker: {}",
                        ticket_id.as_str(),
                        current_worker
                    );
                    Ok(ClaimResult::AlreadyClaimed(current_worker))
                }
                Some((state, _, dep_status)) => {
                    info!(
                        "Ticket {} not claimable: state={}, dependency_status={}",
                        ticket_id.as_str(),
                        state,
                        dep_status
                    );
                    Ok(ClaimResult::NotClaimable {
                        state,
                        dependency_status: dep_status,
                    })
                }
                None => Err(anyhow::anyhow!("Ticket {} not found", ticket_id.as_str())),
            };
        }

        tx.commit().await?;
        info!(
            "Successfully claimed ticket {} for worker {}",
            ticket_id.as_str(),
            worker_id
        );
        Ok(ClaimResult::Success)
    }

    /// Release a ticket claim if it's currently claimed
    pub async fn release_ticket_if_claimed(db: &DbPool, ticket_id: &TicketId) -> Result<()> {
        // First check if ticket is claimed
        let is_claimed = sqlx::query_scalar::<_, bool>(
            "SELECT processing_worker_id IS NOT NULL FROM tickets WHERE ticket_id = ?1",
        )
        .bind(ticket_id.as_str())
        .fetch_optional(db)
        .await?
        .unwrap_or(false);

        if is_claimed {
            info!("Releasing claim on ticket: {}", ticket_id);

            sqlx::query(
                "UPDATE tickets SET processing_worker_id = NULL, updated_at = datetime('now') WHERE ticket_id = ?1"
            )
            .bind(ticket_id.as_str())
            .execute(db)
            .await?;

            // Ticket claim released (no event needed - redundant)

            info!("Successfully released claim on ticket: {}", ticket_id);
        } else {
            info!("Ticket {} was not claimed, no need to release", ticket_id);
        }

        Ok(())
    }

    /// Release a specific ticket claim by ticket ID
    pub async fn release_ticket_claim(db: &DbPool, ticket_id: &str) -> Result<()> {
        info!("Releasing claim for ticket: {}", ticket_id);

        let rows_affected = sqlx::query(
            "UPDATE tickets SET processing_worker_id = NULL, updated_at = datetime('now') WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL"
        )
        .bind(ticket_id)
        .execute(db)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            info!("Successfully released claim for ticket: {}", ticket_id);

            // Ticket claim released (no event needed - redundant)
        } else {
            info!("No active claim found for ticket: {}", ticket_id);
        }

        Ok(())
    }

    /// Release a specific ticket claim by ticket ID and worker ID (scoped release)
    pub async fn release_ticket_claim_for_worker(
        db: &DbPool,
        ticket_id: &str,
        worker_id: &str,
    ) -> Result<()> {
        info!(
            "Releasing claim for ticket: {} by worker: {}",
            ticket_id, worker_id
        );

        let rows_affected = sqlx::query(
            "UPDATE tickets SET processing_worker_id = NULL, updated_at = datetime('now') WHERE ticket_id = ?1 AND processing_worker_id = ?2"
        )
        .bind(ticket_id)
        .bind(worker_id)
        .execute(db)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            info!(
                "Successfully released claim on ticket: {} by worker: {}",
                ticket_id, worker_id
            );
        } else {
            info!(
                "No matching claim found for ticket: {} by worker: {}",
                ticket_id, worker_id
            );
        }

        Ok(())
    }

    /// Emergency release of all claimed tickets (used during shutdown or errors)
    pub async fn emergency_release_claimed_tickets(db: &DbPool) -> Result<()> {
        warn!("Emergency release of all claimed tickets");

        let claimed_tickets = sqlx::query_scalar::<_, String>(
            "SELECT ticket_id FROM tickets WHERE processing_worker_id IS NOT NULL",
        )
        .fetch_all(db)
        .await?;

        if claimed_tickets.is_empty() {
            info!("No claimed tickets to release");
            return Ok(());
        }

        info!("Releasing {} claimed tickets", claimed_tickets.len());

        for ticket_id in &claimed_tickets {
            if let Err(e) = Self::release_ticket_claim(db, ticket_id).await {
                error!("Failed to release claim for ticket {}: {}", ticket_id, e);
            }
        }

        // Batch update all remaining claims
        let rows_affected = sqlx::query(
            "UPDATE tickets SET processing_worker_id = NULL, updated_at = datetime('now') WHERE processing_worker_id IS NOT NULL"
        )
        .execute(db)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            warn!(
                "Emergency released {} additional ticket claims",
                rows_affected
            );
        }

        // Publish emergency release event
        // Emergency claim release completed (no event needed - redundant)

        Ok(())
    }
}
