//! Knowledge management handlers

use crate::{Error, Result};
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_core::knowledge::{AccessLevel, KnowledgeEntry, KnowledgeType};
use vibe_ensemble_storage::StorageManager;

/// Form data for creating knowledge entries
#[derive(Debug, Deserialize)]
pub struct KnowledgeForm {
    pub title: String,
    pub content: String,
    pub category: String,
    pub knowledge_type: String,
    pub access_level: String,
    pub tags: String, // Comma-separated tags
}

/// List all knowledge entries
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    let knowledge_entries = storage.knowledge().list().await?;

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
    let knowledge = storage
        .knowledge()
        .find_by_id(id)
        .await?
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

/// Show new knowledge entry form
pub async fn new_form() -> Result<Html<String>> {
    let html = r#"
        <html>
            <head><title>New Knowledge Entry</title></head>
            <body>
                <h1>Create New Knowledge Entry</h1>
                <form method="post" action="/knowledge">
                    <p>
                        <label>Title: <input type="text" name="title" required style="width: 400px;"></label>
                    </p>
                    <p>
                        <label>Category: <input type="text" name="category" required placeholder="e.g., rust, patterns, best-practices"></label>
                    </p>
                    <p>
                        <label>Knowledge Type: 
                            <select name="knowledge_type">
                                <option value="Pattern">Pattern</option>
                                <option value="Practice">Practice</option>
                                <option value="Guideline">Guideline</option>
                                <option value="Example">Example</option>
                            </select>
                        </label>
                    </p>
                    <p>
                        <label>Access Level: 
                            <select name="access_level">
                                <option value="Public">Public</option>
                                <option value="Internal">Internal</option>
                                <option value="Restricted">Restricted</option>
                            </select>
                        </label>
                    </p>
                    <p>
                        <label>Tags: <input type="text" name="tags" placeholder="comma,separated,tags"></label>
                    </p>
                    <p>
                        <label>Content:<br>
                            <textarea name="content" required rows="15" cols="80" placeholder="Enter the knowledge content here..."></textarea>
                        </label>
                    </p>
                    <p>
                        <input type="submit" value="Create Knowledge Entry">
                        <a href="/knowledge" style="margin-left: 20px;">Cancel</a>
                    </p>
                </form>
            </body>
        </html>
    "#;

    Ok(Html(html.to_string()))
}

/// Create a new knowledge entry
pub async fn create(
    State(storage): State<Arc<StorageManager>>,
    Form(form): Form<KnowledgeForm>,
) -> Result<Html<String>> {
    let knowledge_type = match form.knowledge_type.as_str() {
        "Pattern" => KnowledgeType::Pattern,
        "Practice" => KnowledgeType::Practice,
        "Guideline" => KnowledgeType::Guideline,
        "Example" => KnowledgeType::Example,
        _ => return Err(Error::BadRequest("Invalid knowledge type".to_string())),
    };

    let access_level = match form.access_level.as_str() {
        "Public" => AccessLevel::Public,
        "Internal" => AccessLevel::Internal,
        "Restricted" => AccessLevel::Restricted,
        _ => return Err(Error::BadRequest("Invalid access level".to_string())),
    };

    // Parse tags from comma-separated string
    let tags: Vec<String> = form
        .tags
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let knowledge_entry = KnowledgeEntry::new(
        form.title.clone(),
        form.content,
        knowledge_type,
        access_level,
        form.category,
        tags,
    )?;

    storage.knowledge().create(&knowledge_entry).await?;

    let html = format!(
        r#"
        <html>
            <head>
                <title>Knowledge Entry Created</title>
                <meta http-equiv="refresh" content="2;url=/knowledge">
            </head>
            <body>
                <h1>Knowledge Entry Created Successfully!</h1>
                <p>Entry: {}</p>
                <p>Redirecting to knowledge base...</p>
                <a href="/knowledge">Go to Knowledge Base</a>
            </body>
        </html>
        "#,
        form.title
    );

    Ok(Html(html))
}
