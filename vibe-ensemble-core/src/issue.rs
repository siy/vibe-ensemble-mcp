//! Issue domain model and related types
//!
//! This module provides the core issue model for representing tasks and work items
//! in the Vibe Ensemble system. Issues track work that needs to be completed by agents.
//!
//! # Examples
//!
//! Creating a new issue:
//!
//! ```rust
//! use vibe_ensemble_core::issue::*;
//!
//! let issue = Issue::builder()
//!     .title("Implement user authentication")
//!     .description("Add OAuth2 authentication to the API")
//!     .priority(IssuePriority::High)
//!     .tag("backend")
//!     .tag("security")
//!     .build()
//!     .unwrap();
//! ```

use crate::{Error, Result};
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
    pub knowledge_links: Vec<String>,
    pub web_metadata: Option<WebMetadata>,
}

/// Web metadata associated with an issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebMetadata {
    pub github_url: Option<String>,
    pub pull_request_url: Option<String>,
    pub external_refs: Vec<String>,
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
    /// Create a new issue with validation
    pub fn new(title: String, description: String, priority: IssuePriority) -> Result<Self> {
        Self::validate_title(&title)?;
        Self::validate_description(&description)?;

        let now = Utc::now();
        Ok(Self {
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
            knowledge_links: Vec::new(),
            web_metadata: None,
        })
    }

    /// Create a builder for constructing an Issue
    pub fn builder() -> IssueBuilder {
        IssueBuilder::new()
    }

    /// Validate issue title
    fn validate_title(title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Err(Error::Validation {
                message: "Issue title cannot be empty".to_string(),
            });
        }
        if title.len() > 200 {
            return Err(Error::Validation {
                message: "Issue title cannot exceed 200 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Validate issue description
    fn validate_description(description: &str) -> Result<()> {
        if description.trim().is_empty() {
            return Err(Error::Validation {
                message: "Issue description cannot be empty".to_string(),
            });
        }
        if description.len() > 10000 {
            return Err(Error::Validation {
                message: "Issue description cannot exceed 10000 characters".to_string(),
            });
        }
        Ok(())
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

    /// Add a tag to the issue
    pub fn add_tag(&mut self, tag: String) -> Result<()> {
        if tag.trim().is_empty() {
            return Err(Error::Validation {
                message: "Tag cannot be empty".to_string(),
            });
        }
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Remove a tag from the issue
    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Add a knowledge link to the issue
    pub fn add_knowledge_link(&mut self, link: String) -> Result<()> {
        if link.trim().is_empty() {
            return Err(Error::Validation {
                message: "Knowledge link cannot be empty".to_string(),
            });
        }
        if !self.knowledge_links.contains(&link) {
            self.knowledge_links.push(link);
            self.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Set web metadata for the issue
    pub fn set_web_metadata(&mut self, metadata: WebMetadata) {
        self.web_metadata = Some(metadata);
        self.updated_at = Utc::now();
    }

    /// Update the issue priority
    pub fn set_priority(&mut self, priority: IssuePriority) {
        if self.priority != priority {
            self.priority = priority;
            self.updated_at = Utc::now();
        }
    }

    /// Update the issue status with validation
    pub fn set_status(&mut self, status: IssueStatus) -> Result<()> {
        // Validate status transitions
        match (&self.status, &status) {
            (IssueStatus::Closed, IssueStatus::Open) => {
                return Err(Error::state_transition("Cannot reopen a closed issue"));
            }
            (IssueStatus::Resolved, IssueStatus::InProgress) => {
                return Err(Error::state_transition(
                    "Cannot move resolved issue back to in progress",
                ));
            }
            (IssueStatus::Resolved, IssueStatus::Open) => {
                return Err(Error::state_transition("Cannot reopen a resolved issue"));
            }
            _ => {}
        }

        self.status = status;
        self.updated_at = Utc::now();

        // Set resolved_at when resolving
        if matches!(self.status, IssueStatus::Resolved) {
            self.resolved_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Block the issue with a reason
    pub fn block(&mut self, reason: String) -> Result<()> {
        if reason.trim().is_empty() {
            return Err(Error::validation("Block reason cannot be empty"));
        }
        self.status = IssueStatus::Blocked { reason };
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Unblock the issue and return to open status
    pub fn unblock(&mut self) -> Result<()> {
        if !matches!(self.status, IssueStatus::Blocked { .. }) {
            return Err(Error::state_transition(
                "Cannot unblock an issue that is not blocked",
            ));
        }
        self.status = IssueStatus::Open;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Close the issue
    pub fn close(&mut self) {
        self.status = IssueStatus::Closed;
        self.updated_at = Utc::now();
        if self.resolved_at.is_none() {
            self.resolved_at = Some(Utc::now());
        }
    }

    /// Get the issue's current state description
    pub fn state_description(&self) -> String {
        match &self.status {
            IssueStatus::Open => "Open and ready for assignment".to_string(),
            IssueStatus::InProgress => "Currently being worked on".to_string(),
            IssueStatus::Blocked { reason } => format!("Blocked: {}", reason),
            IssueStatus::Resolved => "Resolved and ready for verification".to_string(),
            IssueStatus::Closed => "Closed and completed".to_string(),
        }
    }

    /// Check if the issue can be assigned
    pub fn can_be_assigned(&self) -> bool {
        matches!(self.status, IssueStatus::Open)
    }

    /// Get time to resolution in seconds (if resolved)
    pub fn time_to_resolution_seconds(&self) -> Option<i64> {
        self.resolved_at.map(|resolved| {
            resolved
                .signed_duration_since(self.created_at)
                .num_seconds()
        })
    }

    /// Check if the issue is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, IssueStatus::Resolved | IssueStatus::Closed)
    }

    /// Get the time elapsed since creation in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds()
    }

    /// Check if the issue has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }
}

impl WebMetadata {
    /// Create a new web metadata instance
    pub fn new() -> Self {
        Self {
            github_url: None,
            pull_request_url: None,
            external_refs: Vec::new(),
        }
    }

    /// Set the GitHub URL
    pub fn with_github_url<S: Into<String>>(mut self, url: S) -> Self {
        self.github_url = Some(url.into());
        self
    }

    /// Set the pull request URL
    pub fn with_pull_request_url<S: Into<String>>(mut self, url: S) -> Self {
        self.pull_request_url = Some(url.into());
        self
    }

    /// Add an external reference
    pub fn with_external_ref<S: Into<String>>(mut self, ref_url: S) -> Self {
        self.external_refs.push(ref_url.into());
        self
    }
}

impl Default for WebMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing Issue instances with validation
#[derive(Debug, Clone)]
pub struct IssueBuilder {
    title: Option<String>,
    description: Option<String>,
    priority: Option<IssuePriority>,
    tags: Vec<String>,
    knowledge_links: Vec<String>,
    web_metadata: Option<WebMetadata>,
}

impl IssueBuilder {
    /// Create a new issue builder
    pub fn new() -> Self {
        Self {
            title: None,
            description: None,
            priority: None,
            tags: Vec::new(),
            knowledge_links: Vec::new(),
            web_metadata: None,
        }
    }

    /// Set the issue title
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the issue description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the issue priority
    pub fn priority(mut self, priority: IssuePriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Add a tag
    pub fn tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Add a knowledge link
    pub fn knowledge_link<S: Into<String>>(mut self, link: S) -> Self {
        self.knowledge_links.push(link.into());
        self
    }

    /// Set web metadata
    pub fn web_metadata(mut self, metadata: WebMetadata) -> Self {
        self.web_metadata = Some(metadata);
        self
    }

    /// Build the Issue instance
    pub fn build(self) -> Result<Issue> {
        let title = self.title.ok_or_else(|| Error::Validation {
            message: "Issue title is required".to_string(),
        })?;
        let description = self.description.ok_or_else(|| Error::Validation {
            message: "Issue description is required".to_string(),
        })?;
        let priority = self.priority.ok_or_else(|| Error::Validation {
            message: "Issue priority is required".to_string(),
        })?;

        let mut issue = Issue::new(title, description, priority)?;

        // Add tags and knowledge links
        for tag in self.tags {
            issue.add_tag(tag)?;
        }
        for link in self.knowledge_links {
            issue.add_knowledge_link(link)?;
        }

        if let Some(metadata) = self.web_metadata {
            issue.set_web_metadata(metadata);
        }

        Ok(issue)
    }
}

impl Default for IssueBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_creation_with_builder() {
        let issue = Issue::builder()
            .title("Test Issue")
            .description("This is a test issue for validation")
            .priority(IssuePriority::Medium)
            .tag("test")
            .tag("validation")
            .knowledge_link("test-pattern-001")
            .build()
            .unwrap();

        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.priority, IssuePriority::Medium);
        assert_eq!(issue.status, IssueStatus::Open);
        assert_eq!(issue.tags.len(), 2);
        assert!(issue.has_tag("test"));
        assert!(issue.has_tag("validation"));
        assert_eq!(issue.knowledge_links.len(), 1);
        assert!(!issue.is_assigned());
        assert!(!issue.is_terminal());
    }

    #[test]
    fn test_issue_title_validation() {
        // Empty title should fail
        let result = Issue::builder()
            .title("")
            .description("Valid description")
            .priority(IssuePriority::Low)
            .build();
        assert!(result.is_err());

        // Too long title should fail
        let long_title = "a".repeat(201);
        let result = Issue::builder()
            .title(long_title)
            .description("Valid description")
            .priority(IssuePriority::Low)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_issue_description_validation() {
        // Empty description should fail
        let result = Issue::builder()
            .title("Valid Title")
            .description("")
            .priority(IssuePriority::Low)
            .build();
        assert!(result.is_err());

        // Too long description should fail
        let long_description = "a".repeat(10001);
        let result = Issue::builder()
            .title("Valid Title")
            .description(long_description)
            .priority(IssuePriority::Low)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_issue_assignment() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        let agent_id = Uuid::new_v4();

        assert!(!issue.is_assigned());
        issue.assign_to(agent_id);
        assert!(issue.is_assigned());
        assert_eq!(issue.assigned_agent_id, Some(agent_id));
        assert_eq!(issue.status, IssueStatus::InProgress);
    }

    #[test]
    fn test_issue_status_transitions() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        // Valid transitions
        assert!(issue.set_status(IssueStatus::InProgress).is_ok());
        assert!(issue.set_status(IssueStatus::Resolved).is_ok());
        assert!(issue.resolved_at.is_some());

        // Invalid transition: resolved -> in progress
        let result = issue.set_status(IssueStatus::InProgress);
        assert!(result.is_err());

        // Reset for another test
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        issue.set_status(IssueStatus::Closed).unwrap();
        assert!(issue.is_terminal());

        // Invalid transition: closed -> open
        let result = issue.set_status(IssueStatus::Open);
        assert!(result.is_err());
    }

    #[test]
    fn test_issue_tag_operations() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        assert!(!issue.has_tag("test"));

        issue.add_tag("test".to_string()).unwrap();
        assert!(issue.has_tag("test"));

        // Adding duplicate tag should not error
        issue.add_tag("test".to_string()).unwrap();
        assert_eq!(issue.tags.len(), 1);

        // Adding empty tag should fail
        let result = issue.add_tag("".to_string());
        assert!(result.is_err());

        issue.remove_tag("test");
        assert!(!issue.has_tag("test"));
    }

    #[test]
    fn test_issue_knowledge_links() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        assert_eq!(issue.knowledge_links.len(), 0);

        issue.add_knowledge_link("pattern-001".to_string()).unwrap();
        assert_eq!(issue.knowledge_links.len(), 1);

        // Adding duplicate link should not error
        issue.add_knowledge_link("pattern-001".to_string()).unwrap();
        assert_eq!(issue.knowledge_links.len(), 1);

        // Adding empty link should fail
        let result = issue.add_knowledge_link("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_web_metadata() {
        let metadata = WebMetadata::new()
            .with_github_url("https://github.com/example/repo/issues/1")
            .with_pull_request_url("https://github.com/example/repo/pull/1")
            .with_external_ref("https://docs.example.com/api");

        let issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .web_metadata(metadata)
            .build()
            .unwrap();

        assert!(issue.web_metadata.is_some());
        let web_meta = issue.web_metadata.as_ref().unwrap();
        assert!(web_meta.github_url.is_some());
        assert!(web_meta.pull_request_url.is_some());
        assert_eq!(web_meta.external_refs.len(), 1);
    }

    #[test]
    fn test_issue_age() {
        let issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        let age = issue.age_seconds();
        assert!(age >= 0);
        assert!(age < 60); // Should be very recent
    }

    #[test]
    fn test_issue_priority_update() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Low)
            .build()
            .unwrap();

        let initial_updated_at = issue.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        issue.set_priority(IssuePriority::Critical);
        assert_eq!(issue.priority, IssuePriority::Critical);
        assert!(issue.updated_at > initial_updated_at);
    }

    #[test]
    fn test_issue_blocking_and_unblocking() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        // Block the issue
        issue.block("Waiting for dependencies".to_string()).unwrap();
        assert!(matches!(issue.status, IssueStatus::Blocked { .. }));
        assert_eq!(
            issue.state_description(),
            "Blocked: Waiting for dependencies"
        );

        // Unblock the issue
        issue.unblock().unwrap();
        assert_eq!(issue.status, IssueStatus::Open);
        assert!(issue.can_be_assigned());

        // Empty block reason should fail
        let result = issue.block("".to_string());
        assert!(result.is_err());

        // Cannot unblock non-blocked issue
        let result = issue.unblock();
        assert!(result.is_err());
    }

    #[test]
    fn test_issue_closing() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        assert!(issue.resolved_at.is_none());

        issue.close();
        assert_eq!(issue.status, IssueStatus::Closed);
        assert!(issue.resolved_at.is_some());
        assert!(issue.is_terminal());
        assert!(!issue.can_be_assigned());
    }

    #[test]
    fn test_issue_enhanced_status_transitions() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        // Resolve the issue
        issue.set_status(IssueStatus::Resolved).unwrap();
        assert!(issue.resolved_at.is_some());

        // Cannot reopen resolved issue
        let result = issue.set_status(IssueStatus::Open);
        assert!(result.is_err());

        // Reset for another test
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        issue.close();

        // Cannot reopen closed issue
        let result = issue.set_status(IssueStatus::Open);
        assert!(result.is_err());
    }

    #[test]
    fn test_issue_time_to_resolution() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        // No resolution time initially
        assert!(issue.time_to_resolution_seconds().is_none());

        // Resolve the issue
        issue.resolve();
        let resolution_time = issue.time_to_resolution_seconds();
        assert!(resolution_time.is_some());
        assert!(resolution_time.unwrap() >= 0);
    }

    #[test]
    fn test_issue_state_descriptions() {
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .build()
            .unwrap();

        assert_eq!(issue.state_description(), "Open and ready for assignment");

        let agent_id = Uuid::new_v4();
        issue.assign_to(agent_id);
        assert_eq!(issue.state_description(), "Currently being worked on");

        issue.block("Test block".to_string()).unwrap();
        assert_eq!(issue.state_description(), "Blocked: Test block");

        issue.unblock().unwrap();
        issue.resolve();
        assert_eq!(
            issue.state_description(),
            "Resolved and ready for verification"
        );

        issue.close();
        assert_eq!(issue.state_description(), "Closed and completed");
    }
}
