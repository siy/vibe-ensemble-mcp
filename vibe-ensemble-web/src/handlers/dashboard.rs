//! Dashboard handlers

use crate::{templates::DashboardTemplate, Result};
use askama::Template;
use axum::{extract::State, response::Html};
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

/// Dashboard index handler
pub async fn index(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    let stats = storage.stats().await?;

    // Get recent issues (limit to 5)
    let issues = storage.issues().list().await?;
    let recent_issues = if issues.is_empty() {
        None
    } else {
        Some(issues.into_iter().take(5).collect())
    };

    let template = DashboardTemplate::new(stats, recent_issues);
    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
}
