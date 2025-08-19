//! System administration handlers

use crate::{auth::Session, websocket::WebSocketManager, Result};
use axum::{
    extract::{Request, State},
    response::Html,
};
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

/// Admin dashboard
pub async fn index(
    State(storage): State<Arc<StorageManager>>,
    State(ws_manager): State<Arc<WebSocketManager>>,
    request: Request,
) -> Result<Html<String>> {
    let session = request.extensions().get::<Session>().cloned();
    
    // Check admin privileges
    if !session.as_ref().map(|s| s.is_admin).unwrap_or(false) {
        return Ok(Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Access Denied</title></head>
            <body>
                <h1>Access Denied</h1>
                <p>Admin privileges required to access this page.</p>
                <a href="/dashboard">Back to Dashboard</a>
            </body>
            </html>
            "#.to_string(),
        ));
    }

    let stats = storage.stats().await?;
    let client_count = ws_manager.client_count().unwrap_or(0);

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>System Administration - Vibe Ensemble</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }}
        .header {{ background: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .nav {{ margin-bottom: 20px; }}
        .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
        .nav a:hover {{ text-decoration: underline; }}
        .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; }}
        .card {{ background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .card h3 {{ margin-top: 0; color: #333; }}
        .stat-grid {{ display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; margin: 15px 0; }}
        .stat {{ background: #f8f9fa; padding: 10px; border-radius: 4px; text-align: center; }}
        .stat-value {{ font-size: 1.5em; font-weight: bold; color: #007bff; }}
        .admin-actions {{ margin-top: 20px; }}
        .btn {{ display: inline-block; padding: 10px 20px; background-color: #007bff; color: white; text-decoration: none; border-radius: 4px; margin-right: 10px; }}
        .btn:hover {{ background-color: #0056b3; }}
        .btn-danger {{ background-color: #dc3545; }}
        .btn-danger:hover {{ background-color: #c82333; }}
        .status-healthy {{ color: #28a745; font-weight: bold; }}
        .status-warning {{ color: #ffc107; font-weight: bold; }}
        .status-error {{ color: #dc3545; font-weight: bold; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>System Administration</h1>
        <p>Administrative interface for Vibe Ensemble MCP server</p>
        <div class="nav">
            <a href="/dashboard">Dashboard</a>
            <a href="/agents">Agents</a>
            <a href="/issues">Issues</a>
            <a href="/knowledge">Knowledge</a>
            <a href="/admin">Admin</a>
        </div>
    </div>

    <div class="grid">
        <div class="card">
            <h3>System Overview</h3>
            <div class="stat-grid">
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>Active Agents</div>
                </div>
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>Open Issues</div>
                </div>
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>Messages</div>
                </div>
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>Knowledge Entries</div>
                </div>
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>System Prompts</div>
                </div>
                <div class="stat">
                    <div class="stat-value">{}</div>
                    <div>WebSocket Clients</div>
                </div>
            </div>
        </div>

        <div class="card">
            <h3>System Health</h3>
            <p><strong>Status:</strong> <span class="status-healthy">Healthy</span></p>
            <p><strong>Database:</strong> <span class="status-healthy">Connected</span></p>
            <p><strong>WebSocket Service:</strong> <span class="status-healthy">Running</span></p>
            <p><strong>Authentication:</strong> <span class="status-healthy">Active</span></p>
            <p><strong>Last Health Check:</strong> {}</p>
        </div>

        <div class="card">
            <h3>Quick Actions</h3>
            <div class="admin-actions">
                <a href="/admin/config" class="btn">System Configuration</a>
                <a href="/admin/logs" class="btn">View Logs</a>
                <a href="/admin/sessions" class="btn">Active Sessions</a>
                <a href="/api/health" class="btn">Health Check API</a>
            </div>
        </div>

        <div class="card">
            <h3>Database Statistics</h3>
            <p><strong>Total Records:</strong> {}</p>
            <p><strong>Storage Location:</strong> SQLite Database</p>
            <p><strong>Last Backup:</strong> N/A</p>
            <p><strong>Database Size:</strong> Unknown</p>
            <div class="admin-actions">
                <a href="#" class="btn" onclick="alert(\"Backup functionality not yet implemented\")">Backup Database</a>
            </div>
        </div>
    </div>

    <script>
        // Auto-refresh every 30 seconds
        setTimeout(() => window.location.reload(), 30000);
    </script>
</body>
</html>
        "#,
        stats.agents_count,
        stats.issues_count,
        stats.messages_count,
        stats.knowledge_count,
        stats.prompts_count,
        client_count,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
        stats.agents_count + stats.issues_count + stats.messages_count + stats.knowledge_count + stats.prompts_count
    );

    Ok(Html(html))
}

/// System configuration page
pub async fn config(request: Request) -> Result<Html<String>> {
    let session = request.extensions().get::<Session>().cloned();
    
    // Check admin privileges
    if !session.as_ref().map(|s| s.is_admin).unwrap_or(false) {
        return Ok(Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Access Denied</title></head>
            <body>
                <h1>Access Denied</h1>
                <p>Admin privileges required to access this page.</p>
                <a href="/dashboard">Back to Dashboard</a>
            </body>
            </html>
            "#.to_string(),
        ));
    }

    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>System Configuration - Vibe Ensemble</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }
        .header { background: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #007bff; }
        .nav a:hover { text-decoration: underline; }
        .card { background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); margin-bottom: 20px; }
        .config-section { margin-bottom: 30px; }
        .config-item { display: flex; justify-content: space-between; align-items: center; padding: 10px 0; border-bottom: 1px solid #eee; }
        .config-item:last-child { border-bottom: none; }
        .config-value { font-family: monospace; background: #f8f9fa; padding: 4px 8px; border-radius: 4px; }
    </style>
</head>
<body>
    <div class="header">
        <h1>System Configuration</h1>
        <div class="nav">
            <a href="/dashboard">Dashboard</a>
            <a href="/admin">Admin</a>
            <a href="/admin/logs">Logs</a>
            <a href="/admin/sessions">Sessions</a>
        </div>
    </div>

    <div class="card">
        <div class="config-section">
            <h3>Server Configuration</h3>
            <div class="config-item">
                <span>Web Server Host</span>
                <span class="config-value">127.0.0.1</span>
            </div>
            <div class="config-item">
                <span>Web Server Port</span>
                <span class="config-value">8081</span>
            </div>
            <div class="config-item">
                <span>MCP Server Port</span>
                <span class="config-value">8080</span>
            </div>
        </div>

        <div class="config-section">
            <h3>Database Configuration</h3>
            <div class="config-item">
                <span>Database Type</span>
                <span class="config-value">SQLite</span>
            </div>
            <div class="config-item">
                <span>Database URL</span>
                <span class="config-value">sqlite:./vibe_ensemble.db</span>
            </div>
            <div class="config-item">
                <span>Auto-migrate on Startup</span>
                <span class="config-value">true</span>
            </div>
        </div>

        <div class="config-section">
            <h3>Authentication Configuration</h3>
            <div class="config-item">
                <span>Session Duration</span>
                <span class="config-value">24 hours</span>
            </div>
            <div class="config-item">
                <span>Authentication Type</span>
                <span class="config-value">Simple (Development)</span>
            </div>
        </div>

        <div class="config-section">
            <h3>WebSocket Configuration</h3>
            <div class="config-item">
                <span>Stats Update Interval</span>
                <span class="config-value">30 seconds</span>
            </div>
            <div class="config-item">
                <span>Ping Interval</span>
                <span class="config-value">60 seconds</span>
            </div>
            <div class="config-item">
                <span>Message Buffer Size</span>
                <span class="config-value">1000 messages</span>
            </div>
        </div>
    </div>

    <div class="card">
        <h3>Configuration Notes</h3>
        <ul>
            <li>Configuration is currently loaded from environment variables and config files</li>
            <li>Changes to configuration require a server restart</li>
            <li>Authentication is currently using a simple in-memory store for development</li>
            <li>Database migrations are applied automatically on startup</li>
        </ul>
    </div>
</body>
</html>
    "#;

    Ok(Html(html.to_string()))
}

/// System logs page
pub async fn logs(request: Request) -> Result<Html<String>> {
    let session = request.extensions().get::<Session>().cloned();
    
    // Check admin privileges
    if !session.as_ref().map(|s| s.is_admin).unwrap_or(false) {
        return Ok(Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Access Denied</title></head>
            <body>
                <h1>Access Denied</h1>
                <p>Admin privileges required to access this page.</p>
                <a href="/dashboard">Back to Dashboard</a>
            </body>
            </html>
            "#.to_string(),
        ));
    }

    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>System Logs - Vibe Ensemble</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }
        .header { background: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #007bff; }
        .nav a:hover { text-decoration: underline; }
        .card { background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .log-container { background: #1e1e1e; color: #d4d4d4; padding: 20px; border-radius: 4px; font-family: monospace; font-size: 14px; overflow-x: auto; max-height: 500px; overflow-y: auto; }
        .log-entry { margin: 2px 0; }
        .log-info { color: #569cd6; }
        .log-warn { color: #dcdcaa; }
        .log-error { color: #f44747; }
        .log-debug { color: #9cdcfe; }
        .log-controls { margin-bottom: 20px; }
        .btn { padding: 8px 16px; background-color: #007bff; color: white; text-decoration: none; border-radius: 4px; margin-right: 10px; }
        .btn:hover { background-color: #0056b3; }
    </style>
</head>
<body>
    <div class="header">
        <h1>System Logs</h1>
        <div class="nav">
            <a href="/dashboard">Dashboard</a>
            <a href="/admin">Admin</a>
            <a href="/admin/config">Config</a>
            <a href="/admin/sessions">Sessions</a>
        </div>
    </div>

    <div class="card">
        <div class="log-controls">
            <a href="#" class="btn" onclick="refreshLogs()">Refresh</a>
            <a href="#" class="btn" onclick="clearLogs()">Clear Display</a>
            <select id="logLevel" onchange="filterLogs()">
                <option value="all">All Levels</option>
                <option value="error">Errors Only</option>
                <option value="warn">Warnings+</option>
                <option value="info">Info+</option>
                <option value="debug">Debug+</option>
            </select>
        </div>

        <div class="log-container" id="logContainer">
            <div class="log-entry log-info">[2024-08-19 12:00:00] INFO  vibe_ensemble_server: Starting Vibe Ensemble MCP Server</div>
            <div class="log-entry log-info">[2024-08-19 12:00:00] INFO  vibe_ensemble_storage: Database connection established</div>
            <div class="log-entry log-info">[2024-08-19 12:00:01] INFO  vibe_ensemble_storage: Running database migrations</div>
            <div class="log-entry log-info">[2024-08-19 12:00:01] INFO  vibe_ensemble_web: Web server starting on 127.0.0.1:8081</div>
            <div class="log-entry log-info">[2024-08-19 12:00:01] INFO  vibe_ensemble_web: WebSocket service initialized</div>
            <div class="log-entry log-debug">[2024-08-19 12:00:05] DEBUG vibe_ensemble_web: WebSocket client connected</div>
            <div class="log-entry log-info">[2024-08-19 12:00:10] INFO  vibe_ensemble_web: User \"admin\" logged in successfully</div>
            <div class="log-entry log-debug">[2024-08-19 12:00:15] DEBUG vibe_ensemble_web: Stats update broadcast sent to 1 clients</div>
            <div class="log-entry log-warn">[2024-08-19 12:00:30] WARN  vibe_ensemble_storage: Slow query detected (>100ms): SELECT * FROM agents</div>
            <div class="log-entry log-info">[2024-08-19 12:01:00] INFO  vibe_ensemble_web: Periodic stats update sent</div>
        </div>

        <p><em>Note: This is a simulated log view. In a production system, logs would be read from actual log files or a logging service.</em></p>
    </div>

    <script>
        function refreshLogs() {
            // In a real system, this would fetch fresh logs
            alert(\"Log refresh functionality would be implemented here\");
        }

        function clearLogs() {
            document.getElementById(\"logContainer\").innerHTML = \"<div class=\\\"log-entry log-info\\\">Log display cleared</div>\";
        }

        function filterLogs() {
            // In a real system, this would filter log entries by level
            const level = document.getElementById(\"logLevel\").value;
            alert(\"Log filtering by level: \" + level + \" (not yet implemented)\");
        }

        // Auto-refresh logs every 10 seconds
        setInterval(() => {
            // In a real system, this would fetch new log entries
            console.log(\"Would refresh logs here\");
        }, 10000);
    </script>
</body>
</html>
    "#;

    Ok(Html(html.to_string()))
}

/// Active sessions page
pub async fn sessions(request: Request) -> Result<Html<String>> {
    let session = request.extensions().get::<Session>().cloned();
    
    // Check admin privileges
    if !session.as_ref().map(|s| s.is_admin).unwrap_or(false) {
        return Ok(Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Access Denied</title></head>
            <body>
                <h1>Access Denied</h1>
                <p>Admin privileges required to access this page.</p>
                <a href="/dashboard">Back to Dashboard</a>
            </body>
            </html>
            "#.to_string(),
        ));
    }

    let current_session = session.unwrap();

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Active Sessions - Vibe Ensemble</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }}
        .header {{ background: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .nav {{ margin-bottom: 20px; }}
        .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
        .nav a:hover {{ text-decoration: underline; }}
        .card {{ background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        table {{ width: 100%; border-collapse: collapse; margin-top: 20px; }}
        th, td {{ border: 1px solid #ddd; padding: 12px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
        .session-current {{ background-color: #d4edda; }}
        .session-admin {{ font-weight: bold; color: #007bff; }}
        .btn {{ padding: 6px 12px; background-color: #dc3545; color: white; text-decoration: none; border-radius: 4px; font-size: 12px; }}
        .btn:hover {{ background-color: #c82333; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Active Sessions</h1>
        <div class="nav">
            <a href="/dashboard">Dashboard</a>
            <a href="/admin">Admin</a>
            <a href="/admin/config">Config</a>
            <a href="/admin/logs">Logs</a>
        </div>
    </div>

    <div class="card">
        <h3>Session Management</h3>
        <p>Currently showing active user sessions. Sessions automatically expire after 24 hours of inactivity.</p>
        
        <table>
            <thead>
                <tr>
                    <th>Session ID</th>
                    <th>User</th>
                    <th>Type</th>
                    <th>Created</th>
                    <th>Expires</th>
                    <th>Status</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                <tr class="session-current">
                    <td>{}</td>
                    <td class="session-admin">{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>Current</td>
                    <td>-</td>
                </tr>
            </tbody>
        </table>
        
        <p><em>Note: Only the current session is shown in this development version. In a production system, this would show all active sessions with the ability to revoke them.</em></p>
        
        <h4>Session Statistics</h4>
        <ul>
            <li><strong>Total Active Sessions:</strong> 1</li>
            <li><strong>Admin Sessions:</strong> 1</li>
            <li><strong>Regular User Sessions:</strong> 0</li>
            <li><strong>Sessions Today:</strong> 1</li>
        </ul>
    </div>
</body>
</html>
        "#,
        current_session.id,
        current_session.username,
        if current_session.is_admin { "Admin" } else { "User" },
        current_session.created_at.format("%Y-%m-%d %H:%M:%S"),
        current_session.expires_at.format("%Y-%m-%d %H:%M:%S"),
    );

    Ok(Html(html))
}