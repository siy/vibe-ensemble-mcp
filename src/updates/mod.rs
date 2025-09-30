use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::{database::DbPool, sse::EventBroadcaster};

const GITHUB_API_URL: &str = "https://api.github.com/repos/siy/vibe-ensemble-mcp/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub struct UpdateService {
    check_interval: Duration,
    http_client: reqwest::Client,
}

impl UpdateService {
    pub fn new(check_interval_hours: u64) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(format!("vibe-ensemble-mcp/{}", CURRENT_VERSION))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            check_interval: Duration::from_secs(check_interval_hours * 3600),
            http_client,
        }
    }

    /// Check for updates and return the latest version info if available
    async fn check_for_updates(&self) -> Result<Option<(String, String)>> {
        debug!("Checking for updates from GitHub API");

        let response = self.http_client.get(GITHUB_API_URL).send().await?;

        if !response.status().is_success() {
            warn!(
                "GitHub API returned non-success status: {}",
                response.status()
            );
            return Ok(None);
        }

        let release: GithubRelease = response.json().await?;

        // Remove 'v' prefix if present for comparison
        let latest_version = release.tag_name.trim_start_matches('v');
        let current_version = CURRENT_VERSION;

        debug!(
            "Current version: {}, Latest version: {}",
            current_version, latest_version
        );

        // Simple version comparison - if they're different, assume update available
        // In production, should use semver crate for proper comparison
        if latest_version != current_version
            && Self::is_newer_version(current_version, latest_version)
        {
            info!(
                "Update available: {} -> {}",
                current_version, latest_version
            );
            Ok(Some((latest_version.to_string(), release.html_url)))
        } else {
            debug!("Already on latest version");
            Ok(None)
        }
    }

    /// Simple version comparison - returns true if latest > current
    fn is_newer_version(current: &str, latest: &str) -> bool {
        // Parse versions as tuples of (major, minor, patch)
        let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() != 3 {
                return None;
            }
            Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ))
        };

        match (parse_version(current), parse_version(latest)) {
            (Some(curr), Some(lat)) => lat > curr,
            _ => false,
        }
    }

    /// Start periodic update checks in a background task
    pub fn start_periodic_checks(
        self,
        db: DbPool,
        broadcaster: EventBroadcaster,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Perform immediate check on startup
            if let Err(e) = self.perform_check(&db, &broadcaster).await {
                error!("Initial update check failed: {}", e);
            }

            // Then check periodically
            loop {
                sleep(self.check_interval).await;

                if let Err(e) = self.perform_check(&db, &broadcaster).await {
                    error!("Periodic update check failed: {}", e);
                }
            }
        })
    }

    /// Perform a single update check and emit appropriate events
    async fn perform_check(&self, db: &DbPool, broadcaster: &EventBroadcaster) -> Result<()> {
        // Create event emitter for this check
        let emitter = crate::events::emitter::EventEmitter::new(db, broadcaster);

        // Emit check started event
        if let Err(e) = emitter.emit_update_check_started(CURRENT_VERSION).await {
            warn!("Failed to emit update_check_started event: {}", e);
        }

        match self.check_for_updates().await {
            Ok(Some((latest_version, release_url))) => {
                // Update available
                if let Err(e) = emitter
                    .emit_update_available(CURRENT_VERSION, &latest_version, &release_url)
                    .await
                {
                    error!("Failed to emit update_available event: {}", e);
                }
            }
            Ok(None) => {
                // No update available - no event needed
                debug!("No update available");
            }
            Err(e) => {
                // Check failed
                let error_msg = format!("Update check failed: {}", e);
                warn!("{}", error_msg);
                if let Err(e) = emitter
                    .emit_update_check_failed(CURRENT_VERSION, &error_msg)
                    .await
                {
                    error!("Failed to emit update_check_failed event: {}", e);
                }
            }
        }

        Ok(())
    }
}
