//! Askama templates for the web interface

use askama::Template;
use vibe_ensemble_core::{agent::Agent, issue::Issue};
use vibe_ensemble_storage::DatabaseStats;

/// Dashboard template
#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub stats: DatabaseStats,
    pub recent_issues: Option<Vec<Issue>>,
    pub current_page: String,
}

impl DashboardTemplate {
    pub fn new(stats: DatabaseStats, recent_issues: Option<Vec<Issue>>) -> Self {
        Self {
            stats,
            recent_issues,
            current_page: "dashboard".to_string(),
        }
    }
}

/// Agents list template
#[derive(Template)]
#[template(path = "agents_list.html")]
pub struct AgentsTemplate {
    pub agents: Vec<Agent>,
    pub current_page: String,
}

impl AgentsTemplate {
    pub fn new(agents: Vec<Agent>) -> Self {
        Self {
            agents,
            current_page: "agents".to_string(),
        }
    }
}
