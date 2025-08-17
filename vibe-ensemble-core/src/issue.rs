//! Issue domain model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an issue/task in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: IssueStatus,
    pub priority: IssuePriority,
    pub assigned_agent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

/// Status of an issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueStatus {
    Open,
    InProgress,
    Blocked { reason: String },
    Resolved,
    Closed,
}

/// Priority level of an issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssuePriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Issue {
    /// Create a new issue
    pub fn new(title: String, description: String, priority: IssuePriority) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            description,
            status: IssueStatus::Open,
            priority,
            assigned_agent_id: None,
            created_at: now,
            updated_at: now,
            resolved_at: None,
            tags: Vec::new(),
        }
    }

    /// Assign the issue to an agent
    pub fn assign_to(&mut self, agent_id: Uuid) {
        self.assigned_agent_id = Some(agent_id);
        self.status = IssueStatus::InProgress;
        self.updated_at = Utc::now();
    }

    /// Mark the issue as resolved
    pub fn resolve(&mut self) {
        self.status = IssueStatus::Resolved;
        let now = Utc::now();
        self.updated_at = now;
        self.resolved_at = Some(now);
    }

    /// Check if the issue is currently assigned
    pub fn is_assigned(&self) -> bool {
        self.assigned_agent_id.is_some()
    }
}