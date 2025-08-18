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
    let issues = storage.issues().list().await?;

    let mut html = String::from(
        r#"
        <html>
            <head><title>Issues</title></head>
            <body>
                <h1>Issues</h1>
                <a href="/issues/new">Create New Issue</a>
                <table border="1">
                    <tr>
                        <th>Title</th>
                        <th>Status</th>
                        <th>Priority</th>
                        <th>Created</th>
                        <th>Actions</th>
                    </tr>
        "#,
    );

    for issue in issues {
        html.push_str(&format!(
            r#"
            <tr>
                <td>{}</td>
                <td>{:?}</td>
                <td>{:?}</td>
                <td>{}</td>
                <td><a href="/issues/{}">View</a></td>
            </tr>
            "#,
            issue.title,
            issue.status,
            issue.priority,
            issue.created_at.format("%Y-%m-%d %H:%M"),
            issue.id
        ));
    }

    html.push_str("</table></body></html>");
    Ok(Html(html))
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
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    let html = format!(
        r#"
        <html>
            <head><title>Issue: {}</title></head>
            <body>
                <h1>{}</h1>
                <p><strong>ID:</strong> {}</p>
                <p><strong>Status:</strong> {:?}</p>
                <p><strong>Priority:</strong> {:?}</p>
                <p><strong>Assigned Agent:</strong> {:?}</p>
                <p><strong>Created:</strong> {}</p>
                <p><strong>Updated:</strong> {}</p>
                <p><strong>Description:</strong></p>
                <div>{}</div>
                <p><a href="/issues">Back to Issues</a></p>
            </body>
        </html>
        "#,
        issue.title,
        issue.title,
        issue.id,
        issue.status,
        issue.priority,
        issue.assigned_agent_id,
        issue.created_at.format("%Y-%m-%d %H:%M:%S"),
        issue.updated_at.format("%Y-%m-%d %H:%M:%S"),
        issue.description
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

/// Update an issue (placeholder)
pub async fn update(
    State(_storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
    Form(_form): Form<IssueForm>,
) -> Result<Html<String>> {
    // Placeholder for update logic
    let html = format!(
        r#"
        <html>
            <head><title>Issue Updated</title></head>
            <body>
                <h1>Issue Update</h1>
                <p>Update functionality not implemented yet.</p>
                <p><a href="/issues/{}">Back to Issue</a></p>
            </body>
        </html>
        "#,
        id
    );

    Ok(Html(html))
}
