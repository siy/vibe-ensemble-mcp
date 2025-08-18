//! Simple template rendering for the web interface

use vibe_ensemble_core::agent::Agent;

/// Dashboard template data
pub struct DashboardTemplate {
    pub agents_count: i64,
    pub issues_count: i64,
    pub messages_count: i64,
    pub knowledge_count: i64,
    pub prompts_count: i64,
}

impl DashboardTemplate {
    /// Render the dashboard template as HTML
    pub fn render(&self) -> Result<String, Box<dyn std::error::Error>> {
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Vibe Ensemble Dashboard</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 0; padding: 20px; }}
        .header {{ border-bottom: 1px solid #ccc; padding-bottom: 10px; margin-bottom: 20px; }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 20px; }}
        .stat-card {{ border: 1px solid #ddd; padding: 20px; border-radius: 5px; text-align: center; }}
        .stat-number {{ font-size: 2em; font-weight: bold; color: #007bff; }}
        .nav {{ margin-bottom: 20px; }}
        .nav a {{ margin-right: 20px; text-decoration: none; color: #007bff; }}
        .nav a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Vibe Ensemble MCP Server</h1>
        <p>Coordination hub for multiple Claude Code instances</p>
    </div>
    
    <div class="nav">
        <a href="/dashboard">Dashboard</a>
        <a href="/agents">Agents</a>
        <a href="/issues">Issues</a>
        <a href="/knowledge">Knowledge</a>
        <a href="/api/health">API Health</a>
    </div>
    
    <div class="stats">
        <div class="stat-card">
            <div class="stat-number">{}</div>
            <div>Active Agents</div>
        </div>
        <div class="stat-card">
            <div class="stat-number">{}</div>
            <div>Total Issues</div>
        </div>
        <div class="stat-card">
            <div class="stat-number">{}</div>
            <div>Messages Exchanged</div>
        </div>
        <div class="stat-card">
            <div class="stat-number">{}</div>
            <div>Knowledge Entries</div>
        </div>
        <div class="stat-card">
            <div class="stat-number">{}</div>
            <div>System Prompts</div>
        </div>
    </div>
    
    <div>
        <h2>Quick Actions</h2>
        <ul>
            <li><a href="/issues/new">Create New Issue</a></li>
            <li><a href="/agents">View All Agents</a></li>
            <li><a href="/knowledge">Browse Knowledge Base</a></li>
            <li><a href="/api/stats">View System Statistics</a></li>
        </ul>
    </div>
</body>
</html>"#,
            self.agents_count,
            self.issues_count,
            self.messages_count,
            self.knowledge_count,
            self.prompts_count
        );
        Ok(html)
    }
}

/// Agents list template data
pub struct AgentsTemplate {
    pub agents: Vec<Agent>,
}

impl AgentsTemplate {
    /// Render the agents list template as HTML
    pub fn render(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::from(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Agents - Vibe Ensemble</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 0; padding: 20px; }
        .header { border-bottom: 1px solid #ccc; padding-bottom: 10px; margin-bottom: 20px; }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #007bff; }
        .nav a:hover { text-decoration: underline; }
        table { width: 100%; border-collapse: collapse; }
        th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }
        th { background-color: #f2f2f2; }
        .status-online { color: green; font-weight: bold; }
        .status-offline { color: red; font-weight: bold; }
        .status-busy { color: orange; font-weight: bold; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Agent Management</h1>
        <p>Connected Claude Code instances</p>
    </div>
    
    <div class="nav">
        <a href="/dashboard">Dashboard</a>
        <a href="/agents">Agents</a>
        <a href="/issues">Issues</a>
        <a href="/knowledge">Knowledge</a>
    </div>
    
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
"#,
        );

        for agent in &self.agents {
            html.push_str(&format!(
                r#"
            <tr>
                <td>{}</td>
                <td>{:?}</td>
                <td><span class="status-{}">{:?}</span></td>
                <td>{}</td>
                <td>{}</td>
                <td><a href="/agents/{}">View Details</a></td>
            </tr>
                "#,
                agent.name,
                agent.agent_type,
                format!("{:?}", agent.status).to_lowercase(),
                agent.status,
                agent.capabilities.join(", "),
                agent.last_seen.format("%Y-%m-%d %H:%M"),
                agent.id
            ));
        }

        html.push_str(
            r#"
        </tbody>
    </table>
    
    <div style="text-align: center; margin-top: 50px; color: #666;">
        <h3>No agents connected</h3>
        <p>Agents will appear here once they connect to the MCP server.</p>
    </div>
</body>
</html>
"#,
        );

        Ok(html)
    }
}
