//! Knowledge management handlers

use crate::{Error, Result};
use axum::{extract::{Path, State}, response::Html};
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

/// List all knowledge entries
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    // For now, we'll create a dummy agent ID to list accessible knowledge
    let dummy_agent_id = Uuid::new_v4();
    let knowledge_entries = storage.knowledge().list_accessible_by(dummy_agent_id).await?;
    
    let mut html = String::from(
        r#"
        <html>
            <head><title>Knowledge Base</title></head>
            <body>
                <h1>Knowledge Base</h1>
                <table border="1">
                    <tr>
                        <th>Title</th>
                        <th>Type</th>
                        <th>Access Level</th>
                        <th>Created</th>
                        <th>Actions</th>
                    </tr>
        "#,
    );
    
    for knowledge in knowledge_entries {
        html.push_str(&format!(
            r#"
            <tr>
                <td>{}</td>
                <td>{:?}</td>
                <td>{:?}</td>
                <td>{}</td>
                <td><a href="/knowledge/{}">View</a></td>
            </tr>
            "#,
            knowledge.title,
            knowledge.knowledge_type,
            knowledge.access_level,
            knowledge.created_at.format("%Y-%m-%d %H:%M"),
            knowledge.id
        ));
    }
    
    html.push_str("</table></body></html>");
    Ok(Html(html))
}

/// Show knowledge entry details
pub async fn detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let knowledge = storage.knowledge().find_by_id(id).await?
        .ok_or_else(|| Error::NotFound(format!("Knowledge entry with id {}", id)))?;
    
    let html = format!(
        r#"
        <html>
            <head><title>Knowledge: {}</title></head>
            <body>
                <h1>{}</h1>
                <p><strong>ID:</strong> {}</p>
                <p><strong>Type:</strong> {:?}</p>
                <p><strong>Access Level:</strong> {:?}</p>
                <p><strong>Tags:</strong> {:?}</p>
                <p><strong>Version:</strong> {}</p>
                <p><strong>Created:</strong> {}</p>
                <p><strong>Updated:</strong> {}</p>
                <div style="border: 1px solid #ccc; padding: 10px; margin: 10px 0;">
                    <pre>{}</pre>
                </div>
                <p><a href="/knowledge">Back to Knowledge Base</a></p>
            </body>
        </html>
        "#,
        knowledge.title,
        knowledge.title,
        knowledge.id,
        knowledge.knowledge_type,
        knowledge.access_level,
        knowledge.tags,
        knowledge.version,
        knowledge.created_at.format("%Y-%m-%d %H:%M:%S"),
        knowledge.updated_at.format("%Y-%m-%d %H:%M:%S"),
        knowledge.content
    );
    
    Ok(Html(html))
}