//! Issue service for business logic and workflow management

use crate::{repositories::IssueRepository, Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;
use vibe_ensemble_core::issue::{Issue, IssuePriority, IssueStatus, WebMetadata};

/// Service for managing issues and workflows
pub struct IssueService {
    repository: Arc<IssueRepository>,
}

/// Statistics about issues in the system
#[derive(Debug, Clone)]
pub struct IssueStatistics {
    pub total_issues: i64,
    pub open_issues: i64,
    pub in_progress_issues: i64,
    pub blocked_issues: i64,
    pub resolved_issues: i64,
    pub closed_issues: i64,
    pub issues_by_priority: HashMap<String, i64>,
    pub assigned_issues: i64,
    pub unassigned_issues: i64,
}

/// Issue assignment recommendation
#[derive(Debug, Clone)]
pub struct AssignmentRecommendation {
    pub agent_id: Uuid,
    pub confidence: f64,
    pub reason: String,
}

/// Workflow transition result
#[derive(Debug, Clone)]
pub struct WorkflowTransition {
    pub from_status: IssueStatus,
    pub to_status: IssueStatus,
    pub allowed: bool,
    pub reason: Option<String>,
}

impl IssueService {
    /// Create a new issue service
    pub fn new(repository: Arc<IssueRepository>) -> Self {
        Self { repository }
    }

    /// Create a new issue with validation
    pub async fn create_issue(
        &self,
        title: String,
        description: String,
        priority: IssuePriority,
        tags: Vec<String>,
    ) -> Result<Issue> {
        info!("Creating new issue: {}", title);

        // Create issue using builder
        let mut issue_builder = Issue::builder()
            .title(title.clone())
            .description(description)
            .priority(priority);

        // Add tags
        for tag in tags {
            issue_builder = issue_builder.tag(tag);
        }

        let issue = issue_builder.build()?;

        // Store in database
        self.repository.create(&issue).await?;

        info!("Successfully created issue: {} ({})", issue.title, issue.id);
        Ok(issue)
    }

    /// Get issue by ID
    pub async fn get_issue(&self, id: Uuid) -> Result<Option<Issue>> {
        debug!("Retrieving issue: {}", id);
        self.repository.find_by_id(id).await
    }

    /// Update an existing issue
    pub async fn update_issue(&self, issue: &Issue) -> Result<()> {
        info!("Updating issue: {} ({})", issue.title, issue.id);
        self.repository.update(issue).await
    }

    /// Delete an issue
    pub async fn delete_issue(&self, id: Uuid) -> Result<()> {
        info!("Deleting issue: {}", id);
        self.repository.delete(id).await
    }

    /// List all issues
    pub async fn list_issues(&self) -> Result<Vec<Issue>> {
        debug!("Listing all issues");
        self.repository.list().await
    }

    /// Get issues by status
    pub async fn get_issues_by_status(&self, status: &IssueStatus) -> Result<Vec<Issue>> {
        debug!("Getting issues by status: {:?}", status);
        self.repository.find_by_status(status).await
    }

    /// Get issues by priority
    pub async fn get_issues_by_priority(&self, priority: &IssuePriority) -> Result<Vec<Issue>> {
        debug!("Getting issues by priority: {:?}", priority);
        self.repository.find_by_priority(priority).await
    }

    /// Get issues assigned to a specific agent
    pub async fn get_agent_issues(&self, agent_id: Uuid) -> Result<Vec<Issue>> {
        debug!("Getting issues for agent: {}", agent_id);
        self.repository.find_by_assigned_agent(agent_id).await
    }

    /// Assign an issue to an agent
    pub async fn assign_issue(&self, issue_id: Uuid, agent_id: Uuid) -> Result<Issue> {
        info!("Assigning issue {} to agent {}", issue_id, agent_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Check if issue can be assigned
        if !issue.can_be_assigned() {
            return Err(Error::InvalidOperation(format!(
                "Issue {} cannot be assigned in current status: {:?}",
                issue_id, issue.status
            )));
        }

        // Assign the issue
        issue.assign_to(agent_id);

        // Update in database
        self.repository.update(&issue).await?;

        info!(
            "Successfully assigned issue {} to agent {}",
            issue_id, agent_id
        );
        Ok(issue)
    }

    /// Unassign an issue
    pub async fn unassign_issue(&self, issue_id: Uuid) -> Result<Issue> {
        info!("Unassigning issue {}", issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Only allow unassigning if in progress
        match issue.status {
            IssueStatus::InProgress => {
                issue.assigned_agent_id = None;
                issue.status = IssueStatus::Open;
                issue.updated_at = chrono::Utc::now();
            }
            _ => {
                return Err(Error::InvalidOperation(format!(
                    "Issue {} cannot be unassigned in current status: {:?}",
                    issue_id, issue.status
                )));
            }
        }

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully unassigned issue {}", issue_id);
        Ok(issue)
    }

    /// Change issue status with validation
    pub async fn change_status(&self, issue_id: Uuid, new_status: IssueStatus) -> Result<Issue> {
        info!("Changing status of issue {} to {:?}", issue_id, new_status);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Validate transition
        let transition = self.validate_status_transition(&issue.status, &new_status);
        if !transition.allowed {
            return Err(Error::InvalidOperation(
                transition
                    .reason
                    .unwrap_or_else(|| "Invalid status transition".to_string()),
            ));
        }

        // Apply status change
        issue.set_status(new_status)?;

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully changed issue {} status", issue_id);
        Ok(issue)
    }

    /// Block an issue with a reason
    pub async fn block_issue(&self, issue_id: Uuid, reason: String) -> Result<Issue> {
        info!("Blocking issue {} with reason: {}", issue_id, reason);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Block the issue
        issue.block(reason)?;

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully blocked issue {}", issue_id);
        Ok(issue)
    }

    /// Unblock an issue
    pub async fn unblock_issue(&self, issue_id: Uuid) -> Result<Issue> {
        info!("Unblocking issue {}", issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Unblock the issue
        issue.unblock()?;

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully unblocked issue {}", issue_id);
        Ok(issue)
    }

    /// Resolve an issue
    pub async fn resolve_issue(&self, issue_id: Uuid) -> Result<Issue> {
        info!("Resolving issue {}", issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Resolve the issue
        issue.resolve();

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully resolved issue {}", issue_id);
        Ok(issue)
    }

    /// Close an issue
    pub async fn close_issue(&self, issue_id: Uuid) -> Result<Issue> {
        info!("Closing issue {}", issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Close the issue
        issue.close();

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully closed issue {}", issue_id);
        Ok(issue)
    }

    /// Update issue priority
    pub async fn update_priority(&self, issue_id: Uuid, priority: IssuePriority) -> Result<Issue> {
        info!("Updating priority of issue {} to {:?}", issue_id, priority);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Update priority
        issue.set_priority(priority);

        // Update in database
        self.repository.update(&issue).await?;

        info!("Successfully updated priority of issue {}", issue_id);
        Ok(issue)
    }

    /// Add tag to an issue
    pub async fn add_tag(&self, issue_id: Uuid, tag: String) -> Result<Issue> {
        debug!("Adding tag '{}' to issue {}", tag, issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Add tag
        issue.add_tag(tag)?;

        // Update in database
        self.repository.update(&issue).await?;

        debug!("Successfully added tag to issue {}", issue_id);
        Ok(issue)
    }

    /// Remove tag from an issue
    pub async fn remove_tag(&self, issue_id: Uuid, tag: &str) -> Result<Issue> {
        debug!("Removing tag '{}' from issue {}", tag, issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Remove tag
        issue.remove_tag(tag);

        // Update in database
        self.repository.update(&issue).await?;

        debug!("Successfully removed tag from issue {}", issue_id);
        Ok(issue)
    }

    /// Add knowledge link to an issue
    pub async fn add_knowledge_link(&self, issue_id: Uuid, link: String) -> Result<Issue> {
        debug!("Adding knowledge link '{}' to issue {}", link, issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Add knowledge link
        issue.add_knowledge_link(link)?;

        // Update in database
        self.repository.update(&issue).await?;

        debug!("Successfully added knowledge link to issue {}", issue_id);
        Ok(issue)
    }

    /// Set web metadata for an issue
    pub async fn set_web_metadata(&self, issue_id: Uuid, metadata: WebMetadata) -> Result<Issue> {
        debug!("Setting web metadata for issue {}", issue_id);

        let mut issue = self
            .get_issue(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Set web metadata
        issue.set_web_metadata(metadata);

        // Update in database
        self.repository.update(&issue).await?;

        debug!("Successfully set web metadata for issue {}", issue_id);
        Ok(issue)
    }

    /// Get issue statistics
    pub async fn get_statistics(&self) -> Result<IssueStatistics> {
        debug!("Computing issue statistics");

        let total_issues = self.repository.count().await?;
        let all_issues = self.repository.list().await?;

        let mut open_issues = 0;
        let mut in_progress_issues = 0;
        let mut blocked_issues = 0;
        let mut resolved_issues = 0;
        let mut closed_issues = 0;
        let mut assigned_issues = 0;
        let mut issues_by_priority = HashMap::new();

        for issue in &all_issues {
            match issue.status {
                IssueStatus::Open => open_issues += 1,
                IssueStatus::InProgress => in_progress_issues += 1,
                IssueStatus::Blocked { .. } => blocked_issues += 1,
                IssueStatus::Resolved => resolved_issues += 1,
                IssueStatus::Closed => closed_issues += 1,
            }

            if issue.is_assigned() {
                assigned_issues += 1;
            }

            let priority_key = format!("{:?}", issue.priority);
            *issues_by_priority.entry(priority_key).or_insert(0) += 1;
        }

        let unassigned_issues = total_issues - assigned_issues;

        let stats = IssueStatistics {
            total_issues,
            open_issues,
            in_progress_issues,
            blocked_issues,
            resolved_issues,
            closed_issues,
            issues_by_priority,
            assigned_issues,
            unassigned_issues,
        };

        debug!("Issue statistics computed: {:?}", stats);
        Ok(stats)
    }

    /// Get open issues (available for assignment)
    pub async fn get_available_issues(&self) -> Result<Vec<Issue>> {
        debug!("Getting available issues for assignment");
        self.repository.find_by_status(&IssueStatus::Open).await
    }

    /// Get high priority issues that need attention
    pub async fn get_priority_issues(&self) -> Result<Vec<Issue>> {
        debug!("Getting high priority issues");

        let mut priority_issues = Vec::new();

        let critical_issues = self
            .repository
            .find_by_priority(&IssuePriority::Critical)
            .await?;
        let high_issues = self
            .repository
            .find_by_priority(&IssuePriority::High)
            .await?;

        priority_issues.extend(critical_issues);
        priority_issues.extend(high_issues);

        // Sort by priority (Critical first, then High) and creation time
        priority_issues.sort_by(|a, b| match (&a.priority, &b.priority) {
            (IssuePriority::Critical, IssuePriority::High) => std::cmp::Ordering::Less,
            (IssuePriority::High, IssuePriority::Critical) => std::cmp::Ordering::Greater,
            _ => a.created_at.cmp(&b.created_at),
        });

        debug!("Found {} high priority issues", priority_issues.len());
        Ok(priority_issues)
    }

    /// Validate status transition
    fn validate_status_transition(
        &self,
        from: &IssueStatus,
        to: &IssueStatus,
    ) -> WorkflowTransition {
        let allowed = match (from, to) {
            // Open can go to any status
            (IssueStatus::Open, _) => true,

            // InProgress can go to resolved, blocked, or back to open
            (IssueStatus::InProgress, IssueStatus::Resolved) => true,
            (IssueStatus::InProgress, IssueStatus::Blocked { .. }) => true,
            (IssueStatus::InProgress, IssueStatus::Open) => true,

            // Blocked can be unblocked to open or resolved
            (IssueStatus::Blocked { .. }, IssueStatus::Open) => true,
            (IssueStatus::Blocked { .. }, IssueStatus::Resolved) => true,

            // Resolved can only be closed
            (IssueStatus::Resolved, IssueStatus::Closed) => true,

            // Closed is terminal
            (IssueStatus::Closed, _) => false,

            // Any other transition
            _ => false,
        };

        let reason = if !allowed {
            Some(format!("Cannot transition from {:?} to {:?}", from, to))
        } else {
            None
        };

        WorkflowTransition {
            from_status: from.clone(),
            to_status: to.clone(),
            allowed,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    include!("issue_tests.rs");
}
