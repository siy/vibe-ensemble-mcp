//! Agent token management handlers

use crate::{Error, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_security::{
    AgentToken, AuthService, CreateAgentTokenRequest, Permission, SecurityError, User,
};
use vibe_ensemble_storage::StorageManager;

/// Agent token handlers
pub struct TokenHandlers {
    pub auth_service: Arc<AuthService>,
    pub storage: Arc<StorageManager>,
}

/// Token creation form data
#[derive(Deserialize)]
pub struct CreateTokenForm {
    pub agent_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_days: Option<i32>,
}

/// Token list query parameters
#[derive(Deserialize)]
pub struct TokenQuery {
    pub agent_id: Option<String>,
    pub active_only: Option<bool>,
}

/// Token response for API
#[derive(Serialize)]
pub struct TokenResponse {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub token: Option<String>, // Only included in create response
}

impl TokenHandlers {
    pub fn new(auth_service: Arc<AuthService>, storage: Arc<StorageManager>) -> Self {
        Self {
            auth_service,
            storage,
        }
    }

    /// Show agent tokens management page
    pub async fn tokens_page(
        State(handlers): State<Arc<TokenHandlers>>,
        Query(query): Query<TokenQuery>,
    ) -> Result<Html<String>> {
        // Get all agents for the dropdown
        let agents = handlers.storage.agents().list().await?;

        // Get tokens (filtered by agent if specified)
        let tokens = handlers
            .list_tokens_internal(query.agent_id.clone(), query.active_only)
            .await?;

        let html = format!(
            r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Agent Tokens - Vibe Ensemble</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            margin: 0;
            padding: 2rem;
            background-color: #f8f9fa;
        }}
        
        .header {{
            background: white;
            padding: 2rem;
            border-radius: 8px;
            margin-bottom: 2rem;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        
        .header h1 {{
            margin: 0 0 0.5rem 0;
            color: #333;
        }}
        
        .header p {{
            margin: 0;
            color: #666;
        }}
        
        .create-section {{
            background: white;
            padding: 2rem;
            border-radius: 8px;
            margin-bottom: 2rem;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        
        .create-section h2 {{
            margin: 0 0 1rem 0;
            color: #333;
        }}
        
        .form-row {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1rem;
            margin-bottom: 1rem;
        }}
        
        .form-group {{
            margin-bottom: 1rem;
        }}
        
        .form-group label {{
            display: block;
            margin-bottom: 0.5rem;
            color: #333;
            font-weight: 500;
        }}
        
        .form-group input,
        .form-group select,
        .form-group textarea {{
            width: 100%;
            padding: 0.75rem;
            border: 2px solid #e1e5e9;
            border-radius: 6px;
            box-sizing: border-box;
        }}
        
        .permissions-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 0.5rem;
            margin-top: 0.5rem;
        }}
        
        .permission-item {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }}
        
        .permission-item input[type="checkbox"] {{
            width: auto;
        }}
        
        .btn {{
            padding: 0.75rem 1.5rem;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 1rem;
        }}
        
        .btn:hover {{
            transform: translateY(-1px);
        }}
        
        .btn-danger {{
            background: linear-gradient(135deg, #ff6b6b 0%, #ee5a52 100%);
        }}
        
        .tokens-section {{
            background: white;
            padding: 2rem;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        
        .tokens-section h2 {{
            margin: 0 0 1rem 0;
            color: #333;
        }}
        
        .tokens-table {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 1rem;
        }}
        
        .tokens-table th,
        .tokens-table td {{
            padding: 0.75rem;
            text-align: left;
            border-bottom: 1px solid #e1e5e9;
        }}
        
        .tokens-table th {{
            background-color: #f8f9fa;
            font-weight: 600;
            color: #555;
        }}
        
        .status-badge {{
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
            font-size: 0.875rem;
            font-weight: 500;
        }}
        
        .status-active {{
            background-color: #d1f7c4;
            color: #365314;
        }}
        
        .status-inactive {{
            background-color: #fed7d7;
            color: #c53030;
        }}
        
        .status-expired {{
            background-color: #feebc8;
            color: #c05621;
        }}
        
        .actions {{
            display: flex;
            gap: 0.5rem;
        }}
        
        .btn-small {{
            padding: 0.375rem 0.75rem;
            font-size: 0.875rem;
            border-radius: 4px;
            border: none;
            cursor: pointer;
        }}
        
        .btn-revoke {{
            background-color: #ef4444;
            color: white;
        }}
        
        .no-tokens {{
            text-align: center;
            padding: 2rem;
            color: #666;
            font-style: italic;
        }}
        
        .filter-section {{
            margin-bottom: 1rem;
            display: flex;
            gap: 1rem;
            align-items: end;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Agent Tokens</h1>
        <p>Manage API tokens for agent authentication</p>
    </div>

    <div class="create-section">
        <h2>Create New Token</h2>
        <form method="post" action="/api/tokens">
            <div class="form-row">
                <div class="form-group">
                    <label for="agent_id">Agent:</label>
                    <select id="agent_id" name="agent_id" required>
                        <option value="">Select an agent...</option>
                        {}
                    </select>
                </div>
                <div class="form-group">
                    <label for="name">Token Name:</label>
                    <input type="text" id="name" name="name" required placeholder="e.g. Production API Access">
                </div>
            </div>
            
            <div class="form-group">
                <label for="expires_days">Expires (days from now):</label>
                <input type="number" id="expires_days" name="expires_days" placeholder="Leave empty for no expiration" min="1" max="3650">
            </div>
            
            <div class="form-group">
                <label>Permissions:</label>
                <div class="permissions-grid">
                    {}
                </div>
            </div>
            
            <button type="submit" class="btn">Create Token</button>
        </form>
    </div>

    <div class="tokens-section">
        <h2>Existing Tokens</h2>
        
        <div class="filter-section">
            <div class="form-group">
                <label for="filter_agent">Filter by Agent:</label>
                <select id="filter_agent" onchange="filterTokens()">
                    <option value="">All agents</option>
                    {}
                </select>
            </div>
            <div class="form-group">
                <label>
                    <input type="checkbox" id="active_only" onchange="filterTokens()" {}> Active only
                </label>
            </div>
        </div>
        
        {}
    </div>

    <script>
        function filterTokens() {{
            const agentFilter = document.getElementById('filter_agent').value;
            const activeOnly = document.getElementById('active_only').checked;
            
            const params = new URLSearchParams();
            if (agentFilter) params.append('agent_id', agentFilter);
            if (activeOnly) params.append('active_only', 'true');
            
            window.location.href = '/tokens?' + params.toString();
        }}
        
        function revokeToken(tokenId) {{
            if (confirm('Are you sure you want to revoke this token? This action cannot be undone.')) {{
                fetch(`/api/tokens/${{tokenId}}`, {{
                    method: 'DELETE',
                    headers: {{
                        'Content-Type': 'application/json',
                    }}
                }})
                .then(response => {{
                    if (response.ok) {{
                        location.reload();
                    }} else {{
                        alert('Failed to revoke token');
                    }}
                }})
                .catch(err => {{
                    alert('Error: ' + err.message);
                }});
            }}
        }}
        
        function copyToken(token) {{
            navigator.clipboard.writeText(token).then(() => {{
                alert('Token copied to clipboard!');
            }});
        }}
    </script>
</body>
</html>
            "#,
            // Agent options for create form
            agents
                .iter()
                .map(|agent| format!(r#"<option value="{}">{}</option>"#, agent.id, agent.name))
                .collect::<Vec<_>>()
                .join(""),
            // Permission checkboxes
            get_all_permissions()
                .iter()
                .map(|perm| format!(
                    r#"<div class="permission-item">
                        <input type="checkbox" id="perm_{}" name="permissions" value="{:?}">
                        <label for="perm_{}">{:?}</label>
                    </div>"#,
                    perm, perm, perm, perm
                ))
                .collect::<Vec<_>>()
                .join(""),
            // Agent options for filter
            agents
                .iter()
                .map(|agent| format!(
                    r#"<option value="{}" {}>{}</option>"#,
                    agent.id,
                    if query.agent_id.as_ref() == Some(&agent.id.to_string()) {
                        "selected"
                    } else {
                        ""
                    },
                    agent.name
                ))
                .collect::<Vec<_>>()
                .join(""),
            // Active only checkbox
            if query.active_only.unwrap_or(false) {
                "checked"
            } else {
                ""
            },
            // Tokens table
            if tokens.is_empty() {
                r#"<div class="no-tokens">No tokens found.</div>"#.to_string()
            } else {
                format!(
                    r#"
                    <table class="tokens-table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Agent</th>
                                <th>Status</th>
                                <th>Last Used</th>
                                <th>Expires</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {}
                        </tbody>
                    </table>
                    "#,
                    tokens.iter()
                        .map(|token| {
                            let agent_name = agents.iter()
                                .find(|a| a.id.to_string() == token.agent_id)
                                .map(|a| a.name.as_str())
                                .unwrap_or("Unknown");
                            let status = if !token.is_active {
                                r#"<span class="status-badge status-inactive">Inactive</span>"#
                            } else if token.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false) {
                                r#"<span class="status-badge status-expired">Expired</span>"#
                            } else {
                                r#"<span class="status-badge status-active">Active</span>"#
                            };
                            let last_used = token.last_used_at
                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "Never".to_string());
                            let expires = token.expires_at
                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "Never".to_string());
                            format!(
                                r#"
                                <tr>
                                    <td>{}</td>
                                    <td>{}</td>
                                    <td>{}</td>
                                    <td>{}</td>
                                    <td>{}</td>
                                    <td class="actions">
                                        <button class="btn-small btn-revoke" onclick="revokeToken('{}')">
                                            Revoke
                                        </button>
                                    </td>
                                </tr>
                                "#,
                                token.name,
                                agent_name,
                                status,
                                last_used,
                                expires,
                                token.id
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("")
                )
            }
        );

        Ok(Html(html))
    }

    /// Create a new agent token (API endpoint)
    pub async fn create_token(
        State(handlers): State<Arc<TokenHandlers>>,
        Json(request): Json<CreateAgentTokenRequest>,
    ) -> Result<(StatusCode, Json<TokenResponse>)> {
        // FUTURE: Get current user from authentication middleware when implemented
        let current_user_id = "admin_user_001"; // This should come from auth

        // Validate agent exists
        let agent = handlers
            .storage
            .agents()
            .find_by_id(
                uuid::Uuid::parse_str(&request.agent_id)
                    .map_err(|_| Error::BadRequest("Invalid agent ID".to_string()))?,
            )
            .await?
            .ok_or_else(|| Error::NotFound("Agent not found".to_string()))?;

        // Parse permissions
        let permissions: Vec<Permission> = request
            .permissions
            .into_iter()
            .filter_map(|p| match p {
                Permission::ViewDashboard => Some(p),
                Permission::ViewAgents => Some(p),
                Permission::ViewIssues => Some(p),
                Permission::CreateIssue => Some(p),
                Permission::UpdateOwnIssue => Some(p),
                Permission::SendMessage => Some(p),
                Permission::CreateKnowledge => Some(p),
                _ => None, // Filter out admin-only permissions for agents
            })
            .collect();

        // Generate JWT token for the agent
        let jwt_token = handlers
            .auth_service
            .jwt_manager()
            .generate_agent_token(
                &request.agent_id,
                permissions.iter().map(|p| format!("{:?}", p)).collect(),
            )
            .map_err(|e| Error::Internal(format!("Failed to generate token: {}", e)))?;

        // Hash the token for storage
        let token_hash = bcrypt::hash(&jwt_token, bcrypt::DEFAULT_COST)
            .map_err(|e| Error::Internal(format!("Failed to hash token: {}", e)))?;

        // Create token record
        let token_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = request.expires_at;

        // Store in database using raw SQL (since we don't have a repository yet)
        sqlx::query!(
            r#"
            INSERT INTO agent_tokens (
                id, agent_id, token_hash, name, permissions, is_active,
                expires_at, created_at, created_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            token_id,
            request.agent_id,
            token_hash,
            request.name,
            serde_json::to_string(&permissions).unwrap(),
            true,
            expires_at,
            now,
            current_user_id
        )
        .execute(handlers.storage.pool())
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let response = TokenResponse {
            id: token_id,
            name: request.name,
            agent_id: request.agent_id,
            permissions: permissions
                .into_iter()
                .map(|p| format!("{:?}", p))
                .collect(),
            is_active: true,
            expires_at,
            last_used_at: None,
            created_at: now,
            token: Some(jwt_token), // Only return token on creation
        };

        Ok((StatusCode::CREATED, Json(response)))
    }

    /// Create token from web form
    pub async fn create_token_form(
        State(handlers): State<Arc<TokenHandlers>>,
        Form(form): Form<CreateTokenForm>,
    ) -> Result<Response> {
        let expires_at = form
            .expires_days
            .map(|days| Utc::now() + chrono::Duration::days(days as i64));

        let permissions: Vec<Permission> = form
            .permissions
            .into_iter()
            .filter_map(|p| serde_json::from_str(&format!("\"{}\"", p)).ok())
            .collect();

        let request = CreateAgentTokenRequest {
            agent_id: form.agent_id,
            name: form.name,
            permissions,
            expires_at,
        };

        match handlers.create_token_internal(request).await {
            Ok((token_response, jwt_token)) => {
                // Show success page with token
                let success_html = format!(
                    r#"
                    <!DOCTYPE html>
                    <html>
                    <head>
                        <title>Token Created - Vibe Ensemble</title>
                        <style>
                            body {{ font-family: Arial, sans-serif; padding: 2rem; }}
                            .success {{ background: #d1f7c4; padding: 2rem; border-radius: 8px; margin-bottom: 2rem; }}
                            .token {{ background: #f5f5f5; padding: 1rem; border-radius: 4px; font-family: monospace; word-break: break-all; }}
                            .warning {{ background: #feebc8; padding: 1rem; border-radius: 4px; margin-top: 1rem; color: #c05621; }}
                            .btn {{ display: inline-block; padding: 0.75rem 1.5rem; background: #667eea; color: white; text-decoration: none; border-radius: 6px; }}
                        </style>
                    </head>
                    <body>
                        <div class="success">
                            <h2>Token Created Successfully</h2>
                            <p><strong>Token Name:</strong> {}</p>
                            <p><strong>Agent ID:</strong> {}</p>
                            <p><strong>Token:</strong></p>
                            <div class="token">{}</div>
                            <div class="warning">
                                <strong>Important:</strong> This token will only be shown once. Please copy it now and store it securely.
                            </div>
                        </div>
                        <a href="/tokens" class="btn">Back to Tokens</a>
                        <script>
                            function copyToken() {{
                                navigator.clipboard.writeText('{}').then(() => {{
                                    alert('Token copied to clipboard!');
                                }});
                            }}
                        </script>
                        <button onclick="copyToken()" class="btn" style="margin-left: 1rem;">Copy Token</button>
                    </body>
                    </html>
                    "#,
                    token_response.name, token_response.agent_id, jwt_token, jwt_token
                );

                Ok(Html(success_html).into_response())
            }
            Err(e) => {
                Ok(axum::response::Redirect::to(&format!("/tokens?error={}", e)).into_response())
            }
        }
    }

    /// List tokens (API endpoint)
    pub async fn list_tokens(
        State(handlers): State<Arc<TokenHandlers>>,
        Query(query): Query<TokenQuery>,
    ) -> Result<Json<Value>> {
        let tokens = handlers
            .list_tokens_internal(query.agent_id, query.active_only)
            .await?;

        Ok(Json(json!({
            "tokens": tokens,
            "count": tokens.len(),
            "timestamp": Utc::now()
        })))
    }

    /// Revoke a token
    pub async fn revoke_token(
        State(handlers): State<Arc<TokenHandlers>>,
        Path(token_id): Path<String>,
    ) -> Result<StatusCode> {
        sqlx::query!(
            "UPDATE agent_tokens SET is_active = 0 WHERE id = ?",
            token_id
        )
        .execute(handlers.storage.pool())
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(StatusCode::NO_CONTENT)
    }

    /// Internal helper to create token
    async fn create_token_internal(
        &self,
        request: CreateAgentTokenRequest,
    ) -> Result<(TokenResponse, String)> {
        // FUTURE: Get current user from authentication middleware when implemented
        let current_user_id = "admin_user_001";

        // Generate JWT token
        let jwt_token = self
            .auth_service
            .jwt_manager()
            .generate_agent_token(
                &request.agent_id,
                request
                    .permissions
                    .iter()
                    .map(|p| format!("{:?}", p))
                    .collect(),
            )
            .map_err(|e| Error::Internal(format!("Failed to generate token: {}", e)))?;

        // Hash the token for storage
        let token_hash = bcrypt::hash(&jwt_token, bcrypt::DEFAULT_COST)
            .map_err(|e| Error::Internal(format!("Failed to hash token: {}", e)))?;

        // Create token record
        let token_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO agent_tokens (
                id, agent_id, token_hash, name, permissions, is_active,
                expires_at, created_at, created_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            token_id,
            request.agent_id,
            token_hash,
            request.name,
            serde_json::to_string(&request.permissions).unwrap(),
            true,
            request.expires_at,
            now,
            current_user_id
        )
        .execute(self.storage.pool())
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let response = TokenResponse {
            id: token_id,
            name: request.name,
            agent_id: request.agent_id,
            permissions: request
                .permissions
                .into_iter()
                .map(|p| format!("{:?}", p))
                .collect(),
            is_active: true,
            expires_at: request.expires_at,
            last_used_at: None,
            created_at: now,
            token: None,
        };

        Ok((response, jwt_token))
    }

    /// Internal helper to list tokens
    async fn list_tokens_internal(
        &self,
        agent_id: Option<String>,
        active_only: Option<bool>,
    ) -> Result<Vec<TokenResponse>> {
        let mut query = "SELECT * FROM agent_tokens WHERE 1=1".to_string();
        let mut params = Vec::new();

        if let Some(agent_id) = agent_id {
            query.push_str(" AND agent_id = ?");
            params.push(agent_id);
        }

        if active_only.unwrap_or(false) {
            query.push_str(" AND is_active = 1");
        }

        query.push_str(" ORDER BY created_at DESC");

        let mut query_builder = sqlx::query(&query);
        for param in params {
            query_builder = query_builder.bind(param);
        }

        let rows = query_builder
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let mut tokens = Vec::new();
        for row in rows {
            let permissions: Vec<Permission> =
                serde_json::from_str(row.get("permissions")).unwrap_or_default();

            tokens.push(TokenResponse {
                id: row.get("id"),
                name: row.get("name"),
                agent_id: row.get("agent_id"),
                permissions: permissions
                    .into_iter()
                    .map(|p| format!("{:?}", p))
                    .collect(),
                is_active: row.get("is_active"),
                expires_at: row.get("expires_at"),
                last_used_at: row.get("last_used_at"),
                created_at: row.get("created_at"),
                token: None, // Never return the actual token in list
            });
        }

        Ok(tokens)
    }
}

/// Get all available permissions for token creation
fn get_all_permissions() -> Vec<Permission> {
    vec![
        Permission::ViewDashboard,
        Permission::ViewAgents,
        Permission::ViewIssues,
        Permission::CreateIssue,
        Permission::UpdateOwnIssue,
        Permission::SendMessage,
        Permission::CreateKnowledge,
        Permission::UpdateOwnKnowledge,
    ]
}
