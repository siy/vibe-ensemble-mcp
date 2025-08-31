//! Knowledge management handlers

use crate::{
    csrf::{generate_csrf_token, validate_csrf_form, CsrfFormToken, CsrfToken},
    server::AppState,
    Error, Result,
};
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    Form,
};
use axum_extra::extract::cookie::CookieJar;
use html_escape::encode_text;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_core::knowledge::{AccessLevel, Knowledge, KnowledgeType};
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
    pub csrf_token: String,
}

/// List all knowledge entries (wrapper for compatibility)
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    list_internal(storage).await
}

/// Render the knowledge list HTML
fn render_knowledge_list_html(
    knowledge_entries: Vec<vibe_ensemble_core::knowledge::Knowledge>,
    search_term: Option<&str>,
) -> Result<Html<String>> {
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Knowledge Browser{} - Vibe Ensemble</title>
            <style>
                body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background-color: #f8f9fa; }}
                .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
                .header {{ border-bottom: 1px solid #e9ecef; padding-bottom: 20px; margin-bottom: 30px; }}
                .nav {{ margin-bottom: 20px; }}
                .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
                .nav a:hover {{ text-decoration: underline; }}
                .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 30px; }}
                .stat-card {{ background: #f8f9fa; padding: 20px; border-radius: 8px; text-align: center; }}
                .stat-number {{ font-size: 2rem; font-weight: bold; color: #007bff; }}
                .stat-label {{ color: #6c757d; font-size: 0.875rem; }}
                .card {{ background: white; padding: 20px; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.1); margin-bottom: 20px; }}
                table {{ width: 100%; border-collapse: collapse; }}
                th, td {{ padding: 12px 8px; text-align: left; border-bottom: 1px solid #dee2e6; }}
                th {{ background-color: #f8f9fa; font-weight: 600; }}
                .knowledge-type {{ padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }}
                .type-pattern {{ background: #007bff; color: white; }}
                .type-practice {{ background: #28a745; color: white; }}
                .type-guideline {{ background: #ffc107; color: #343a40; }}
                .type-solution {{ background: #17a2b8; color: white; }}
                .type-reference {{ background: #6f42c1; color: white; }}
                .access-level {{ padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }}
                .access-public {{ background: #28a745; color: white; }}
                .access-team {{ background: #ffc107; color: #343a40; }}
                .access-private {{ background: #dc3545; color: white; }}
                .btn {{ background: #007bff; color: white; padding: 6px 12px; text-decoration: none; border-radius: 4px; font-size: 0.75rem; }}
                .btn:hover {{ background: #0056b3; }}
                .btn-primary {{ background: #28a745; }}
                .btn-primary:hover {{ background: #218838; }}
            </style>
        </head>
        <body>
            <div class="container">
                <div class="header">
                    <h1>Knowledge Browser</h1>
                    <div class="nav">
                        <a href="/dashboard">Dashboard</a>
                        <a href="/agents">Agents</a>
                        <a href="/issues">Issues</a>
                        <a href="/messages">Messages</a>
                        <a href="/knowledge">Knowledge</a>
                    </div>
                </div>
                
                <div class="stats">
                    <div class="stat-card">
                        <div class="stat-number">{}</div>
                        <div class="stat-label">Total Items</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-number">0</div>
                        <div class="stat-label">Public Items</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-number">0</div>
                        <div class="stat-label">Recent Updates</div>
                    </div>
                </div>
                
                <div class="card">
                    <h3>Knowledge Repository <a href="/knowledge/new" class="btn btn-primary" style="float: right;">Add Knowledge</a></h3>
                    {}
                </div>
            </div>
        </body>
        </html>
        "#,
        if let Some(term) = search_term {
            format!(" - Search: {}", encode_text(term))
        } else {
            String::new()
        },
        knowledge_entries.len(),
        if knowledge_entries.is_empty() {
            "<p>No knowledge items found. <a href='/knowledge/new' class='btn btn-primary'>Add First Knowledge Item</a></p>".to_string()
        } else {
            let mut table_html = String::from("<table><tr><th>Title</th><th>Type</th><th>Access Level</th><th>Created</th><th>Actions</th></tr>");

            for entry in knowledge_entries {
                table_html.push_str(&format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td><span class="knowledge-type type-{}">{:?}</span></td>
                        <td><span class="access-level access-{}">{:?}</span></td>
                        <td>{}</td>
                        <td><a href="/knowledge/{}" class="btn">View</a></td>
                    </tr>"#,
                    encode_text(&entry.title),
                    match entry.knowledge_type {
                        vibe_ensemble_core::knowledge::KnowledgeType::Pattern => "pattern",
                        vibe_ensemble_core::knowledge::KnowledgeType::Practice => "practice",
                        vibe_ensemble_core::knowledge::KnowledgeType::Guideline => "guideline",
                        vibe_ensemble_core::knowledge::KnowledgeType::Solution => "solution",
                        vibe_ensemble_core::knowledge::KnowledgeType::Reference => "reference",
                    },
                    entry.knowledge_type, // Debug format is safe
                    match entry.access_level {
                        vibe_ensemble_core::knowledge::AccessLevel::Public => "public",
                        vibe_ensemble_core::knowledge::AccessLevel::Team => "team",
                        vibe_ensemble_core::knowledge::AccessLevel::Private => "private",
                    },
                    entry.access_level, // Debug format is safe
                    entry.created_at.format("%Y-%m-%d %H:%M"), // Formatted timestamp is safe
                    entry.id            // UUID is safe
                ));
            }

            table_html.push_str("</table>");
            table_html
        }
    );

    Ok(Html(html))
}

/// List all knowledge entries (internal implementation)
async fn list_internal(storage: Arc<StorageManager>) -> Result<Html<String>> {
    // Use list_accessible_by with a dummy UUID to get all public and team entries
    let dummy_agent_id = Uuid::new_v4();
    let knowledge_entries = storage
        .knowledge()
        .list_accessible_by(dummy_agent_id)
        .await
        .map_err(crate::Error::Storage)?;

    render_knowledge_list_html(knowledge_entries, None)
}

/// Show knowledge entry details (wrapper for compatibility)
pub async fn detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    detail_internal(storage, id).await
}

/// Show knowledge entry details (internal implementation)
async fn detail_internal(storage: Arc<StorageManager>, id: Uuid) -> Result<Html<String>> {
    // First find the knowledge entry
    let knowledge = storage
        .knowledge()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Knowledge entry with id {}", id)))?;

    // Check access level - only allow access to Public and Team entries
    // In a real implementation, you'd get the current user's agent ID from authentication
    match knowledge.access_level {
        AccessLevel::Public | AccessLevel::Team => {
            // Access allowed
        }
        AccessLevel::Private => {
            // For now, block access to private entries since we don't have authentication
            // TODO: Implement proper authentication and check if user owns this entry
            return Err(Error::Forbidden(
                "Access denied to private knowledge entry".to_string(),
            ));
        }
    }

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
        encode_text(&knowledge.title),                    // HTML title
        encode_text(&knowledge.title),                    // Page header
        knowledge.id,                                     // UUID is safe
        knowledge.knowledge_type,                         // Debug format is safe
        knowledge.access_level,                           // Debug format is safe
        knowledge.tags,                                   // Debug format is safe (Vec<String>)
        knowledge.version,                                // Number is safe
        knowledge.created_at.format("%Y-%m-%d %H:%M:%S"), // Formatted timestamp is safe
        knowledge.updated_at.format("%Y-%m-%d %H:%M:%S"), // Formatted timestamp is safe
        encode_text(&knowledge.content)                   // User content needs escaping
    );

    Ok(Html(html))
}

/// Show new knowledge entry form
pub async fn new_form(State(app_state): State<AppState>) -> Result<impl IntoResponse> {
    // Generate CSRF token
    let (csrf_token, csrf_cookie) = generate_csrf_token(&app_state.csrf_store).await;

    let html = format!(
        r#"
        <html>
            <head><title>New Knowledge Entry</title></head>
            <body>
                <h1>Create New Knowledge Entry</h1>
                <form method="post" action="/knowledge">
                    <input type="hidden" name="csrf_token" value="{}" />
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
                                <option value="Solution">Solution</option>
                                <option value="Reference">Reference</option>
                            </select>
                        </label>
                    </p>
                    <p>
                        <label>Access Level: 
                            <select name="access_level">
                                <option value="Public">Public</option>
                                <option value="Team">Team</option>
                                <option value="Private">Private</option>
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
    "#,
        csrf_token
    );

    Ok((CookieJar::new().add(csrf_cookie), Html(html)))
}

/// Search knowledge entries (wrapper for compatibility)
pub async fn search(
    State(storage): State<Arc<StorageManager>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Html<String>> {
    search_internal(storage, params).await
}

/// Search knowledge entries (internal implementation)
async fn search_internal(
    storage: Arc<StorageManager>,
    params: std::collections::HashMap<String, String>,
) -> Result<Html<String>> {
    let search_term = params.get("q").cloned().unwrap_or_default();

    if search_term.trim().is_empty() {
        // No search term, show all entries
        return list_internal(storage).await;
    }

    let dummy_agent_id = Uuid::new_v4();
    let knowledge_entries = storage
        .knowledge()
        .list_accessible_by(dummy_agent_id)
        .await
        .map_err(crate::Error::Storage)?;

    // Basic text search
    let search_lower = search_term.to_lowercase();
    let matching_entries: Vec<_> = knowledge_entries
        .into_iter()
        .filter(|entry| {
            entry.title.to_lowercase().contains(&search_lower)
                || entry.content.to_lowercase().contains(&search_lower)
                || entry
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&search_lower))
        })
        .collect();

    // Render search results with the same format as list but with filtered entries
    render_knowledge_list_html(matching_entries, Some(&search_term))
}

/// Create a new knowledge entry
pub async fn create(
    State(app_state): State<AppState>,
    cookie_token: Option<CsrfToken>,
    Form(form): Form<KnowledgeForm>,
) -> Result<Html<String>> {
    // Validate CSRF token
    let csrf_form_token = CsrfFormToken {
        csrf_token: form.csrf_token.clone(),
    };

    if let Err(_response) = validate_csrf_form(
        State(app_state.csrf_store.clone()),
        csrf_form_token,
        cookie_token,
    )
    .await
    {
        return Err(Error::Forbidden("Invalid CSRF token".to_string()));
    }
    let knowledge_type = match form.knowledge_type.as_str() {
        "Pattern" => KnowledgeType::Pattern,
        "Practice" => KnowledgeType::Practice,
        "Guideline" => KnowledgeType::Guideline,
        "Solution" => KnowledgeType::Solution,
        "Reference" => KnowledgeType::Reference,
        _ => return Err(Error::BadRequest("Invalid knowledge type".to_string())),
    };

    let access_level = match form.access_level.as_str() {
        "Public" => AccessLevel::Public,
        "Team" => AccessLevel::Team,
        "Private" => AccessLevel::Private,
        _ => return Err(Error::BadRequest("Invalid access level".to_string())),
    };

    // Parse tags from comma-separated string
    let tags: Vec<String> = form
        .tags
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let dummy_created_by = Uuid::new_v4(); // In real implementation, use authenticated user
    let mut knowledge_entry = Knowledge::new(
        form.title.clone(),
        form.content,
        knowledge_type,
        dummy_created_by,
        access_level,
    )?;

    // Add tags
    for tag in tags {
        knowledge_entry.add_tag(tag)?;
    }

    app_state
        .storage
        .knowledge()
        .create(&knowledge_entry)
        .await?;

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
