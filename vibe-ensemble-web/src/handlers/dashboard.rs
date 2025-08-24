//! Dashboard handlers

use crate::{metrics::MetricsCollector, templates::DashboardTemplate, Result};
use askama::Template;
use axum::{extract::State, response::Html};
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

/// Dashboard index handler
pub async fn index(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    // Get basic counts from storage
    let agents = storage.agents().list().await.unwrap_or_default();
    let issues = storage.issues().list().await.unwrap_or_default();
    let active_agents = agents.len();
    let open_issues = issues.len();

    // Get recent issues (limit to 5)
    let recent_issues = if issues.is_empty() {
        None
    } else {
        Some(issues.into_iter().take(5).collect())
    };

    // Collect system and storage metrics
    let metrics_collector = MetricsCollector::new(storage.clone());
    let system_metrics = metrics_collector.collect_system_metrics().await;
    let storage_metrics = metrics_collector.collect_storage_metrics().await;

    let template = DashboardTemplate::new(active_agents, open_issues, recent_issues)
        .with_system_metrics(system_metrics)
        .with_storage_metrics(storage_metrics);
    
    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
}
