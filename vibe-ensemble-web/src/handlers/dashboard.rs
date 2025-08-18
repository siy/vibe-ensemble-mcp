//! Dashboard handlers

use crate::{templates::DashboardTemplate, Result};
use axum::{extract::State, response::Html};
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

/// Dashboard index handler
pub async fn index(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    let stats = storage.stats().await?;

    let template = DashboardTemplate {
        agents_count: stats.agents_count,
        issues_count: stats.issues_count,
        messages_count: stats.messages_count,
        knowledge_count: stats.knowledge_count,
        prompts_count: stats.prompts_count,
    };

    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
}
