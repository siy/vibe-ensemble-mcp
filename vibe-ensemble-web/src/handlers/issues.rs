//! Issue management handlers

use crate::{Error, Result};
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_core::issue::{Issue, IssuePriority};
use vibe_ensemble_storage::StorageManager;

/// Form data for creating/updating issues
#[derive(Debug, Deserialize)]
pub struct IssueForm {
    pub title: String,
    pub description: String,
    pub priority: String,
}

/// List all issues
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    use crate::templates::IssuesListTemplate;
    use askama::Template;

    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    let template = IssuesListTemplate::new(issues);
    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("Template rendering failed: {}", e)))?;

    Ok(Html(rendered))
}

/// Show new issue form
pub async fn new_form() -> Result<Html<String>> {
    let html = r#"
        <html>
            <head><title>New Issue</title></head>
            <body>
                <h1>Create New Issue</h1>
                <form method="post" action="/issues">
                    <p>
                        <label>Title: <input type="text" name="title" required></label>
                    </p>
                    <p>
                        <label>Description: <textarea name="description" required></textarea></label>
                    </p>
                    <p>
                        <label>Priority: 
                            <select name="priority">
                                <option value="Low">Low</option>
                                <option value="Medium">Medium</option>
                                <option value="High">High</option>
                                <option value="Critical">Critical</option>
                            </select>
                        </label>
                    </p>
                    <p>
                        <input type="submit" value="Create Issue">
                    </p>
                </form>
            </body>
        </html>
    "#;

    Ok(Html(html.to_string()))
}

/// Create a new issue
pub async fn create(
    State(storage): State<Arc<StorageManager>>,
    Form(form): Form<IssueForm>,
) -> Result<Html<String>> {
    let priority = match form.priority.as_str() {
        "Low" => IssuePriority::Low,
        "Medium" => IssuePriority::Medium,
        "High" => IssuePriority::High,
        "Critical" => IssuePriority::Critical,
        _ => return Err(Error::BadRequest("Invalid priority".to_string())),
    };

    let issue = Issue::new(form.title, form.description, priority)?;
    storage.issues().create(&issue).await?;

    let html = format!(
        r#"
        <html>
            <head>
                <title>Issue Created</title>
                <meta http-equiv="refresh" content="2;url=/issues">
            </head>
            <body>
                <h1>Issue Created Successfully!</h1>
                <p>Issue ID: {}</p>
                <p>Redirecting to issues list...</p>
                <a href="/issues">Go to Issues</a>
            </body>
        </html>
        "#,
        issue.id
    );

    Ok(Html(html))
}

/// Show issue details
pub async fn detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    // Get assigned agent if any
    let assigned_agent = if let Some(agent_id) = issue.assigned_agent_id {
        storage
            .agents()
            .find_by_id(agent_id)
            .await
            .map_err(crate::Error::Storage)?
    } else {
        None
    };

    // Get related messages
    let messages = storage
        .messages()
        .list_recent(100)
        .await
        .map_err(crate::Error::Storage)?;

    let related_messages: Vec<_> = messages
        .into_iter()
        .filter(|msg| msg.metadata.issue_id == Some(id))
        .take(20)
        .collect();

    // Return simple HTML for now (same pattern as existing)
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Issue: {} - Vibe Ensemble</title>
            <style>
                body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background-color: #f8f9fa; }}
                .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
                .header {{ border-bottom: 1px solid #e9ecef; padding-bottom: 20px; margin-bottom: 30px; }}
                .nav {{ margin-bottom: 20px; }}
                .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
                .nav a:hover {{ text-decoration: underline; }}
                .grid {{ display: grid; grid-template-columns: 2fr 1fr; gap: 30px; margin-bottom: 30px; }}
                .card {{ background: #f8f9fa; padding: 20px; border-radius: 8px; border: 1px solid #dee2e6; margin-bottom: 20px; }}
                .card h3 {{ margin-top: 0; color: #343a40; }}
                .priority {{ padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }}
                .priority-high {{ background: #f8d7da; color: #721c24; }}
                .priority-critical {{ background: #dc3545; color: white; }}
                .priority-medium {{ background: #fff3cd; color: #856404; }}
                .priority-low {{ background: #d4edda; color: #155724; }}
                .status {{ padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }}
                .status-open {{ background: #cce5ff; color: #004085; }}
                .status-inprogress {{ background: #fff3cd; color: #856404; }}
                .status-resolved {{ background: #d4edda; color: #155724; }}
                .status-closed {{ background: #f8f9fa; color: #6c757d; }}
                table {{ width: 100%; border-collapse: collapse; margin-top: 15px; }}
                th, td {{ padding: 8px; text-align: left; border-bottom: 1px solid #dee2e6; }}
                th {{ background-color: #f8f9fa; font-weight: 600; }}
                .btn {{ background: #007bff; color: white; padding: 8px 16px; text-decoration: none; border-radius: 4px; font-size: 0.875rem; margin-right: 10px; }}
                .btn:hover {{ background: #0056b3; }}
                .btn-success {{ background: #28a745; }}
                .btn-success:hover {{ background: #218838; }}
            </style>
        </head>
        <body>
            <div class="container">
                <div class="header">
                    <h1>Issue: {}</h1>
                    <div class="nav">
                        <a href="/dashboard">Dashboard</a>
                        <a href="/agents">Agents</a>
                        <a href="/issues">Issues</a>
                        <a href="/messages">Messages</a>
                        <a href="/knowledge">Knowledge</a>
                    </div>
                </div>
                
                <div class="grid">
                    <div class="card">
                        <h3>Issue Information</h3>
                        <table>
                            <tr><td><strong>ID</strong></td><td>{}</td></tr>
                            <tr><td><strong>Title</strong></td><td>{}</td></tr>
                            <tr><td><strong>Priority</strong></td><td><span class="priority priority-{}">{:?}</span></td></tr>
                            <tr><td><strong>Status</strong></td><td><span class="status status-{}">{:?}</span></td></tr>
                            <tr><td><strong>Created</strong></td><td>{}</td></tr>
                            <tr><td><strong>Last Updated</strong></td><td>{}</td></tr>
                            <tr><td><strong>Assigned Agent</strong></td><td>{}</td></tr>
                        </table>
                    </div>
                    
                    <div class="card">
                        <h3>Actions</h3>
                        <a href="/issues" class="btn">← Back to Issues</a>
                        <a href="/issues/{}/edit" class="btn btn-success">Edit Issue</a>
                    </div>
                </div>
                
                <div class="card">
                    <h3>Description</h3>
                    <div style="background: white; padding: 20px; border-radius: 8px; line-height: 1.6;">
                        {}
                    </div>
                </div>
                
                <div class="card">
                    <h3>Related Messages ({})</h3>
                    {}
                </div>
            </div>
        </body>
        </html>
        "#,
        issue.title,
        issue.title,
        issue.id,
        issue.title,
        match issue.priority {
            vibe_ensemble_core::issue::IssuePriority::High => "high",
            vibe_ensemble_core::issue::IssuePriority::Critical => "critical",
            vibe_ensemble_core::issue::IssuePriority::Medium => "medium",
            vibe_ensemble_core::issue::IssuePriority::Low => "low",
        },
        issue.priority,
        match issue.status {
            vibe_ensemble_core::issue::IssueStatus::Open => "open",
            vibe_ensemble_core::issue::IssueStatus::InProgress => "inprogress",
            vibe_ensemble_core::issue::IssueStatus::Resolved => "resolved",
            vibe_ensemble_core::issue::IssueStatus::Closed => "closed",
            vibe_ensemble_core::issue::IssueStatus::Blocked { .. } => "blocked",
        },
        issue.status,
        issue.created_at.format("%Y-%m-%d %H:%M:%S"),
        issue.updated_at.format("%Y-%m-%d %H:%M:%S"),
        match issue.assigned_agent_id {
            Some(agent_id) => match &assigned_agent {
                Some(agent) => format!("{} ({})", agent.name, agent_id),
                None => format!("{} (Agent not found)", agent_id),
            },
            None => "Unassigned".to_string(),
        },
        issue.id,
        if issue.description.is_empty() {
            "No description provided.".to_string()
        } else {
            issue.description.replace('\n', "<br>")
        },
        related_messages.len(),
        if related_messages.is_empty() {
            "<p>No related messages.</p>".to_string()
        } else {
            related_messages
                .iter()
                .map(|m| {
                    format!(
                        "<p><strong>{:?}</strong>: {}</p>",
                        m.message_type,
                        m.content.chars().take(200).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        }
    );

    Ok(Html(html))
}

/// Show edit issue form (placeholder)
pub async fn edit_form(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let _issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    // Placeholder for edit form
    let html = format!(
        r#"
        <html>
            <head><title>Edit Issue</title></head>
            <body>
                <h1>Edit Issue</h1>
                <p>Edit form not implemented yet.</p>
                <p><a href="/issues/{}">Back to Issue</a></p>
            </body>
        </html>
        "#,
        id
    );

    Ok(Html(html))
}

/// Update an issue
pub async fn update(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
    Form(form): Form<IssueForm>,
) -> Result<Html<String>> {
    let mut issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    let priority = match form.priority.as_str() {
        "Low" => IssuePriority::Low,
        "Medium" => IssuePriority::Medium,
        "High" => IssuePriority::High,
        "Critical" => IssuePriority::Critical,
        _ => return Err(Error::BadRequest("Invalid priority".to_string())),
    };

    // Update issue fields
    issue.title = form.title;
    issue.description = form.description;
    issue.priority = priority;
    issue.updated_at = chrono::Utc::now();

    storage.issues().update(&issue).await?;

    let html = format!(
        r#"
        <html>
            <head>
                <title>Issue Updated</title>
                <meta http-equiv="refresh" content="2;url=/issues/{}">
            </head>
            <body>
                <h1>Issue Updated Successfully!</h1>
                <p>Issue: {}</p>
                <p>Redirecting to issue details...</p>
                <a href="/issues/{}">View Issue</a>
            </body>
        </html>
        "#,
        id, issue.title, id
    );

    Ok(Html(html))
}

/// Delete an issue
pub async fn delete(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    storage.issues().delete(id).await?;

    let html = format!(
        r#"
        <html>
            <head>
                <title>Issue Deleted</title>
                <meta http-equiv="refresh" content="2;url=/issues">
            </head>
            <body>
                <h1>Issue Deleted Successfully!</h1>
                <p>Issue "{}" has been deleted.</p>
                <p>Redirecting to issues list...</p>
                <a href="/issues">Back to Issues</a>
            </body>
        </html>
        "#,
        issue.title
    );

    Ok(Html(html))
}
