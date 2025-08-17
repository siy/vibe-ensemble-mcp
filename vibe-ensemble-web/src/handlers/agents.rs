//! Agent management handlers

use crate::{templates::AgentsTemplate, Error, Result};
use axum::{extract::{Path, State}, response::Html};
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

/// List all agents
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    let agents = storage.agents().list().await?;
    
    let template = AgentsTemplate { agents };
    let rendered = template.render().map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
}

/// Show agent details
pub async fn detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let agent = storage.agents().find_by_id(id).await?
        .ok_or_else(|| Error::NotFound(format!("Agent with id {}", id)))?;
    
    // For now, return a simple HTML representation
    // In practice, you'd use a proper template
    let html = format!(
        r#"
        <html>
            <head><title>Agent Details</title></head>
            <body>
                <h1>Agent: {}</h1>
                <p>ID: {}</p>
                <p>Type: {:?}</p>
                <p>Status: {:?}</p>
                <p>Capabilities: {:?}</p>
                <p>Created: {}</p>
                <p>Last Seen: {}</p>
            </body>
        </html>
        "#,
        agent.name,
        agent.id,
        agent.agent_type,
        agent.status,
        agent.capabilities,
        agent.created_at,
        agent.last_seen
    );
    
    Ok(Html(html))
}