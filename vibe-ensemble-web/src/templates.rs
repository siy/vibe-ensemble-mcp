//! Askama templates for the web dashboard

use askama::Template;
use serde::Serialize;

/// Activity entry for the dashboard
#[derive(Debug, Serialize)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub message: String,
    pub activity_type: String,
}

/// Dashboard template
#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub title: String,
    pub active_agents: usize,
    pub open_issues: usize,
    pub recent_activity: Vec<ActivityEntry>,
    pub current_page: String,
}

impl DashboardTemplate {
    pub fn has_recent_activity(&self) -> bool {
        !self.recent_activity.is_empty()
    }
}
