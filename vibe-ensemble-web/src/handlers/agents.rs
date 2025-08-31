//! Agent management handlers

use crate::{Error, Result};
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    Json,
};
use html_escape::encode_text;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

/// Agent termination request
#[derive(Debug, Deserialize)]
pub struct TerminateAgentRequest {
    pub force: Option<bool>,
    pub reason: Option<String>,
    pub csrf_token: String,
}

/// List all agents
pub async fn list(State(storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    let agents = storage.agents().list().await?;

    let total_count = agents.len();
    let active_count = agents
        .iter()
        .filter(|a| matches!(a.status, vibe_ensemble_core::agent::AgentStatus::Online))
        .count();

    let agents_html = if agents.is_empty() {
        "<p>No agents connected. Agents will appear here once they connect to the MCP server.</p>"
            .to_string()
    } else {
        agents
            .iter()
            .map(|agent| {
                format!(
                    r#"
                <tr>
                    <td><strong>{}</strong><br><small style="color: #666;">{}</small></td>
                    <td><span class="tag">{:?}</span></td>
                    <td><span class="status status-{}">{:?}</span></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td><a href="/agents/{}" class="btn btn-sm">View Details</a></td>
                </tr>
                "#,
                    encode_text(&agent.name),
                    agent.id,         // UUIDs are safe, no need to escape
                    agent.agent_type, // Debug format is safe
                    match agent.status {
                        vibe_ensemble_core::agent::AgentStatus::Online => "online",
                        vibe_ensemble_core::agent::AgentStatus::Offline => "offline",
                        vibe_ensemble_core::agent::AgentStatus::Busy => "busy",
                        _ => "unknown",
                    },
                    agent.status, // Debug format is safe
                    encode_text(&agent.capabilities.join(", ")),
                    agent.last_seen.format("%Y-%m-%d %H:%M"), // Formatted timestamp is safe
                    agent.id                                  // UUIDs are safe, no need to escape
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Agents - Vibe Ensemble</title>
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
                .status {{ padding: 4px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }}
                .status-online {{ background: #d4edda; color: #155724; }}
                .status-offline {{ background: #f8d7da; color: #721c24; }}
                .status-busy {{ background: #fff3cd; color: #856404; }}
                .tag {{ background: #e9ecef; color: #495057; padding: 2px 6px; border-radius: 3px; font-size: 0.75rem; }}
                .btn {{ background: #007bff; color: white; padding: 6px 12px; text-decoration: none; border-radius: 4px; font-size: 0.75rem; }}
                .btn:hover {{ background: #0056b3; }}
                .btn-sm {{ font-size: 0.675rem; padding: 4px 8px; }}
            </style>
        </head>
        <body>
            <div class="container">
                <div class="header">
                    <h1>Agent Management</h1>
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
                        <div class="stat-label">Total Agents</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-number">{}</div>
                        <div class="stat-label">Active Agents</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-number">{}</div>
                        <div class="stat-label">Offline Agents</div>
                    </div>
                </div>
                
                <div class="card">
                    <h3>Connected Agents</h3>
                    {}
                    {}
                </div>
            </div>
        </body>
        </html>
        "#,
        total_count,
        active_count,
        total_count - active_count,
        if agents.is_empty() {
            agents_html
        } else {
            format!(
                r#"
                <table>
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Type</th>
                            <th>Status</th>
                            <th>Capabilities</th>
                            <th>Last Seen</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
                "#,
                agents_html
            )
        },
        ""
    );

    Ok(Html(html))
}

/// Show agent details
pub async fn detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>> {
    let agent = storage
        .agents()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Agent with id {}", id)))?;

    // Get recent messages from/to this agent
    let messages = storage
        .messages()
        .list_recent(100)
        .await
        .map_err(crate::Error::Storage)?;

    let recent_messages: Vec<_> = messages
        .into_iter()
        .filter(|msg| msg.sender_id == id || msg.recipient_id == Some(id))
        .take(10)
        .collect();

    // Get issues assigned to this agent
    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    let assigned_issues: Vec<_> = issues
        .into_iter()
        .filter(|issue| issue.assigned_agent_id == Some(id))
        .collect();

    // For now, return a simple HTML representation with enhanced styling
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Agent: {} - Vibe Ensemble</title>
            <style>
                body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background-color: #f8f9fa; }}
                .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
                .header {{ border-bottom: 1px solid #e9ecef; padding-bottom: 20px; margin-bottom: 30px; }}
                .nav {{ margin-bottom: 20px; }}
                .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
                .nav a:hover {{ text-decoration: underline; }}
                .grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 30px; margin-bottom: 30px; }}
                .card {{ background: #f8f9fa; padding: 20px; border-radius: 8px; border: 1px solid #dee2e6; margin-bottom: 20px; }}
                .card h3 {{ margin-top: 0; color: #343a40; }}
                .status {{ padding: 4px 8px; border-radius: 4px; font-size: 0.875rem; font-weight: 500; }}
                .status-online {{ background: #d4edda; color: #155724; }}
                .status-offline {{ background: #f8d7da; color: #721c24; }}
                .status-busy {{ background: #fff3cd; color: #856404; }}
                .tag {{ background: #e9ecef; color: #495057; padding: 2px 6px; border-radius: 3px; font-size: 0.75rem; margin-right: 4px; }}
                table {{ width: 100%; border-collapse: collapse; margin-top: 15px; }}
                th, td {{ padding: 8px; text-align: left; border-bottom: 1px solid #dee2e6; }}
                th {{ background-color: #f8f9fa; font-weight: 600; }}
                .btn {{ background: #007bff; color: white; padding: 8px 16px; text-decoration: none; border-radius: 4px; font-size: 0.875rem; }}
                .btn:hover {{ background: #0056b3; }}
            </style>
        </head>
        <body>
            <div class="container">
                <div class="header">
                    <h1>Agent: {}</h1>
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
                        <h3>Agent Information</h3>
                        <table>
                            <tr><td><strong>ID</strong></td><td>{}</td></tr>
                            <tr><td><strong>Name</strong></td><td>{}</td></tr>
                            <tr><td><strong>Type</strong></td><td>{:?}</td></tr>
                            <tr><td><strong>Status</strong></td><td><span class="status status-{}">{:?}</span></td></tr>
                            <tr><td><strong>Created</strong></td><td>{}</td></tr>
                            <tr><td><strong>Last Seen</strong></td><td>{}</td></tr>
                        </table>
                    </div>
                    
                    <div class="card">
                        <h3>Capabilities</h3>
                        <div>
                            {}
                        </div>
                    </div>
                </div>
                
                <div class="card">
                    <h3>Assigned Issues ({})</h3>
                    {}
                </div>
                
                <div class="card">
                    <h3>Recent Messages ({})</h3>
                    {}
                </div>
                
                <div class="card">
                    <a href="/agents" class="btn">← Back to Agents</a>
                </div>
            </div>
        </body>
        </html>
        "#,
        encode_text(&agent.name), // Title
        encode_text(&agent.name), // Header
        agent.id,                 // UUID is safe
        encode_text(&agent.name), // Table
        agent.agent_type,         // Debug format is safe
        match agent.status {
            vibe_ensemble_core::agent::AgentStatus::Online => "online",
            vibe_ensemble_core::agent::AgentStatus::Offline => "offline",
            vibe_ensemble_core::agent::AgentStatus::Busy => "busy",
            _ => "unknown",
        },
        agent.status,                                 // Debug format is safe
        agent.created_at.format("%Y-%m-%d %H:%M:%S"), // Formatted timestamp is safe
        agent.last_seen.format("%Y-%m-%d %H:%M:%S"),  // Formatted timestamp is safe
        agent
            .capabilities
            .iter()
            .map(|c| format!("<span class=\"tag\">{}</span>", encode_text(c)))
            .collect::<Vec<_>>()
            .join(" "),
        assigned_issues.len(),
        if assigned_issues.is_empty() {
            "<p>No issues assigned to this agent.</p>".to_string()
        } else {
            assigned_issues
                .iter()
                .map(|i| {
                    format!(
                        "<p><strong>{}</strong> - {:?}</p>",
                        encode_text(&i.title),
                        i.status
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        },
        recent_messages.len(),
        if recent_messages.is_empty() {
            "<p>No recent messages.</p>".to_string()
        } else {
            recent_messages
                .iter()
                .map(|m| {
                    format!(
                        "<p><strong>{:?}</strong>: {}</p>",
                        m.message_type, // Debug format is safe
                        encode_text(&m.content.chars().take(100).collect::<String>())
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        }
    );

    Ok(Html(html))
}

/// Terminate an agent
pub async fn terminate(
    State(app_state): State<crate::server::AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<TerminateAgentRequest>,
) -> Result<impl IntoResponse> {
    // Validate CSRF token directly (API endpoint approach)
    if !app_state
        .csrf_store
        .validate_token(&request.csrf_token)
        .await
    {
        return Err(Error::Forbidden("Invalid CSRF token".to_string()));
    }

    // Verify agent exists
    let agent = app_state
        .storage
        .agents()
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::Error::NotFound(format!("Agent with id {}", id)))?;

    // For now, we'll mark the agent as offline since we don't have direct process control
    // TODO: Implement actual agent termination when process lifecycle is integrated
    app_state
        .storage
        .agents()
        .update_status(id, &vibe_ensemble_core::agent::AgentStatus::Offline)
        .await?;

    // Bound reason length for safety
    let reason = request
        .reason
        .as_deref()
        .unwrap_or("Terminated by user")
        .chars()
        .take(500)
        .collect::<String>();

    tracing::info!(
        "Agent {} ({}) terminated by user request (force: {}, reason: {})",
        agent.name,
        agent.id,
        request.force.unwrap_or(false),
        reason
    );

    use axum::http::StatusCode;
    use serde_json::json;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "success": true,
            "message": format!("Agent {} has been terminated", agent.name),
            "agent_id": agent.id,
            "force": request.force.unwrap_or(false),
            "reason": reason,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    ))
}
