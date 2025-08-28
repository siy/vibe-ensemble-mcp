//! Askama templates for the web dashboard

use askama::Template;
use serde::Serialize;
use vibe_ensemble_core::issue::Issue;

/// Activity entry for the dashboard
#[derive(Debug, Serialize)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub message: String,
    pub activity_type: String,
}

/// System metrics for the dashboard
#[derive(Debug, Serialize, Clone)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_total_mb: u64,
    pub disk_usage_mb: u64,
    pub disk_total_mb: u64,
    pub uptime_seconds: u64,
    pub active_connections: usize,
}

impl SystemMetrics {
    pub fn memory_usage_percent(&self) -> f64 {
        if self.memory_total_mb > 0 {
            let pct = (self.memory_usage_mb as f64 / self.memory_total_mb as f64) * 100.0;
            pct.clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    pub fn memory_usage_percent_int(&self) -> u64 {
        self.memory_usage_percent() as u64
    }

    pub fn disk_usage_percent(&self) -> f64 {
        if self.disk_total_mb > 0 {
            let pct = (self.disk_usage_mb as f64 / self.disk_total_mb as f64) * 100.0;
            pct.clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    pub fn disk_usage_percent_int(&self) -> u64 {
        self.disk_usage_percent() as u64
    }

    pub fn cpu_usage_percent_int(&self) -> u64 {
        self.cpu_usage_percent.round() as u64
    }

    pub fn uptime_hours(&self) -> u64 {
        self.uptime_seconds / 3600
    }

    pub fn uptime_minutes(&self) -> u64 {
        (self.uptime_seconds % 3600) / 60
    }
}

/// Storage health metrics
#[derive(Debug, Serialize, Clone)]
pub struct StorageMetrics {
    pub database_size_mb: u64,
    pub total_queries: u64,
    pub avg_query_time_ms: u64,
    pub active_connections: u32,
    pub max_connections: u32,
}

impl StorageMetrics {
    pub fn connection_usage_percent(&self) -> f64 {
        if self.max_connections > 0 {
            (self.active_connections as f64 / self.max_connections as f64) * 100.0
        } else {
            0.0
        }
    }
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
    pub system_metrics: Option<SystemMetrics>,
    pub storage_metrics: Option<StorageMetrics>,
}

impl DashboardTemplate {
    pub fn new(
        active_agents: usize,
        open_issues: usize,
        recent_issues: Option<Vec<Issue>>,
    ) -> Self {
        let mut recent_activity = Vec::new();

        // Convert recent issues to activity entries
        if let Some(issues) = recent_issues {
            for issue in issues.into_iter().take(5) {
                recent_activity.push(ActivityEntry {
                    timestamp: issue.created_at.format("%H:%M").to_string(),
                    message: format!("Issue created: {}", issue.title),
                    activity_type: "issue".to_string(),
                });
            }
        }

        Self {
            title: "Vibe Ensemble Dashboard".to_string(),
            active_agents,
            open_issues,
            recent_activity,
            current_page: "dashboard".to_string(),
            system_metrics: None, // Will be populated by system metrics collection
            storage_metrics: None, // Will be populated by storage metrics collection
        }
    }

    pub fn with_system_metrics(mut self, metrics: SystemMetrics) -> Self {
        self.system_metrics = Some(metrics);
        self
    }

    pub fn with_storage_metrics(mut self, metrics: StorageMetrics) -> Self {
        self.storage_metrics = Some(metrics);
        self
    }

    pub fn with_recent_activity(mut self, activity: Vec<ActivityEntry>) -> Self {
        self.recent_activity = activity;
        self
    }

    pub fn has_recent_activity(&self) -> bool {
        !self.recent_activity.is_empty()
    }
}

/// Messages template
#[derive(Template)]
#[template(path = "messages.html")]
pub struct MessagesTemplate {
    pub title: String,
    pub message_stats: serde_json::Value,
    pub conversation_count: usize,
    pub current_page: String,
    pub system_metrics: Option<SystemMetrics>,
    pub storage_metrics: Option<StorageMetrics>,
}

impl MessagesTemplate {
    pub fn new(message_stats: serde_json::Value, conversation_count: usize) -> Self {
        Self {
            title: "Messages Dashboard".to_string(),
            message_stats,
            conversation_count,
            current_page: "messages".to_string(),
            system_metrics: None,
            storage_metrics: None,
        }
    }

    pub fn with_system_metrics(mut self, metrics: SystemMetrics) -> Self {
        self.system_metrics = Some(metrics);
        self
    }

    pub fn with_storage_metrics(mut self, metrics: StorageMetrics) -> Self {
        self.storage_metrics = Some(metrics);
        self
    }
}

impl Default for MessagesTemplate {
    fn default() -> Self {
        Self::new(serde_json::json!({}), 0)
    }
}

/// Link Health Dashboard Template
#[derive(Template)]
#[template(path = "link_health.html")]
pub struct LinkHealthTemplate {
    pub current_page: String,
}

impl LinkHealthTemplate {
    pub fn new() -> Self {
        Self {
            current_page: "link_health".to_string(),
        }
    }
}

impl Default for LinkHealthTemplate {
    fn default() -> Self {
        Self::new()
    }
}
