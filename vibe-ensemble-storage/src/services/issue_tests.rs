/// Tests for issue service
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::*;
    use crate::repositories::{IssueRepository, AgentRepository};
    use vibe_ensemble_core::{
        issue::{IssueStatus, IssuePriority, WebMetadata},
        agent::{Agent, AgentType, ConnectionMetadata}
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    async fn setup_test_service() -> (IssueService, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let database_url = format!("sqlite://{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
        
        let repository = Arc::new(IssueRepository::new(pool));
        let service = IssueService::new(repository);
        (service, temp_file)
    }

    async fn setup_test_service_with_agent() -> (IssueService, AgentRepository, Uuid, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let database_url = format!("sqlite://{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
        
        let issue_repository = Arc::new(IssueRepository::new(pool.clone()));
        let agent_repository = AgentRepository::new(pool);
        let service = IssueService::new(issue_repository);
        
        // Create a test agent
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent = Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();
        
        agent_repository.create(&agent).await.expect("Failed to create test agent");
        
        (service, agent_repository, agent.id, temp_file)
    }

    #[tokio::test]
    async fn test_create_issue() {
        let (service, _temp) = setup_test_service().await;

        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string(), "service".to_string()]
        ).await.expect("Failed to create issue");

        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.description, "This is a test issue");
        assert_eq!(issue.priority, IssuePriority::Medium);
        assert_eq!(issue.status, IssueStatus::Open);
        assert!(issue.has_tag("test"));
        assert!(issue.has_tag("service"));
        assert!(!issue.is_assigned());
    }

    #[tokio::test]
    async fn test_get_issue() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue first
        let created_issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::High,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Get the issue
        let retrieved_issue = service.get_issue(created_issue.id).await.expect("Failed to get issue");
        assert!(retrieved_issue.is_some());
        
        let issue = retrieved_issue.unwrap();
        assert_eq!(issue.id, created_issue.id);
        assert_eq!(issue.title, created_issue.title);
        assert_eq!(issue.priority, IssuePriority::High);
    }

    #[tokio::test]
    async fn test_get_nonexistent_issue() {
        let (service, _temp) = setup_test_service().await;
        let non_existent_id = Uuid::new_v4();

        let result = service.get_issue(non_existent_id).await.expect("Query should not fail");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_assign_issue() {
        let (service, _agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Assign the issue
        let assigned_issue = service.assign_issue(issue.id, agent_id).await.expect("Failed to assign issue");

        assert_eq!(assigned_issue.assigned_agent_id, Some(agent_id));
        assert_eq!(assigned_issue.status, IssueStatus::InProgress);
        assert!(assigned_issue.is_assigned());
    }

    #[tokio::test]
    async fn test_assign_non_assignable_issue() {
        let (service, _temp) = setup_test_service().await;
        let agent_id = Uuid::new_v4();

        // Create and resolve an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        let _resolved_issue = service.resolve_issue(issue.id).await.expect("Failed to resolve issue");

        // Try to assign the resolved issue (should fail)
        let result = service.assign_issue(issue.id, agent_id).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidOperation(_) => {}, // Expected
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[tokio::test]
    async fn test_unassign_issue() {
        let (service, _agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;

        // Create and assign an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        let _assigned_issue = service.assign_issue(issue.id, agent_id).await.expect("Failed to assign issue");

        // Unassign the issue
        let unassigned_issue = service.unassign_issue(issue.id).await.expect("Failed to unassign issue");

        assert_eq!(unassigned_issue.assigned_agent_id, None);
        assert_eq!(unassigned_issue.status, IssueStatus::Open);
        assert!(!unassigned_issue.is_assigned());
    }

    #[tokio::test]
    async fn test_change_status() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Change status to InProgress
        let updated_issue = service.change_status(issue.id, IssueStatus::InProgress).await.expect("Failed to change status");
        assert_eq!(updated_issue.status, IssueStatus::InProgress);

        // Change status to Resolved
        let resolved_issue = service.change_status(issue.id, IssueStatus::Resolved).await.expect("Failed to change status");
        assert_eq!(resolved_issue.status, IssueStatus::Resolved);
        assert!(resolved_issue.resolved_at.is_some());
    }

    #[tokio::test]
    async fn test_invalid_status_transition() {
        let (service, _temp) = setup_test_service().await;

        // Create and resolve an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        let _resolved_issue = service.resolve_issue(issue.id).await.expect("Failed to resolve issue");

        // Try to change back to InProgress (should fail)
        let result = service.change_status(issue.id, IssueStatus::InProgress).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidOperation(_) => {}, // Expected
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[tokio::test]
    async fn test_block_and_unblock_issue() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Block the issue
        let blocked_issue = service.block_issue(issue.id, "Waiting for dependencies".to_string()).await.expect("Failed to block issue");
        
        match blocked_issue.status {
            IssueStatus::Blocked { ref reason } => {
                assert_eq!(reason, "Waiting for dependencies");
            },
            _ => panic!("Expected blocked status"),
        }

        // Unblock the issue
        let unblocked_issue = service.unblock_issue(issue.id).await.expect("Failed to unblock issue");
        assert_eq!(unblocked_issue.status, IssueStatus::Open);
    }

    #[tokio::test]
    async fn test_resolve_issue() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Resolve the issue
        let resolved_issue = service.resolve_issue(issue.id).await.expect("Failed to resolve issue");

        assert_eq!(resolved_issue.status, IssueStatus::Resolved);
        assert!(resolved_issue.resolved_at.is_some());
        assert!(resolved_issue.is_terminal());
    }

    #[tokio::test]
    async fn test_close_issue() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Close the issue
        let closed_issue = service.close_issue(issue.id).await.expect("Failed to close issue");

        assert_eq!(closed_issue.status, IssueStatus::Closed);
        assert!(closed_issue.resolved_at.is_some());
        assert!(closed_issue.is_terminal());
    }

    #[tokio::test]
    async fn test_update_priority() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Low,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Update priority
        let updated_issue = service.update_priority(issue.id, IssuePriority::Critical).await.expect("Failed to update priority");

        assert_eq!(updated_issue.priority, IssuePriority::Critical);
    }

    #[tokio::test]
    async fn test_add_and_remove_tag() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["initial".to_string()]
        ).await.expect("Failed to create issue");

        // Add tag
        let tagged_issue = service.add_tag(issue.id, "new-tag".to_string()).await.expect("Failed to add tag");
        assert!(tagged_issue.has_tag("initial"));
        assert!(tagged_issue.has_tag("new-tag"));

        // Remove tag
        let untagged_issue = service.remove_tag(issue.id, "initial").await.expect("Failed to remove tag");
        assert!(!untagged_issue.has_tag("initial"));
        assert!(untagged_issue.has_tag("new-tag"));
    }

    #[tokio::test]
    async fn test_add_knowledge_link() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Add knowledge link
        let updated_issue = service.add_knowledge_link(issue.id, "pattern-001".to_string()).await.expect("Failed to add knowledge link");
        assert_eq!(updated_issue.knowledge_links.len(), 1);
        assert!(updated_issue.knowledge_links.contains(&"pattern-001".to_string()));
    }

    #[tokio::test]
    async fn test_set_web_metadata() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue(
            "Test Issue".to_string(),
            "This is a test issue".to_string(),
            IssuePriority::Medium,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        // Set web metadata
        let metadata = WebMetadata::new()
            .with_github_url("https://github.com/example/repo/issues/1")
            .with_pull_request_url("https://github.com/example/repo/pull/1");

        let updated_issue = service.set_web_metadata(issue.id, metadata).await.expect("Failed to set web metadata");
        
        assert!(updated_issue.web_metadata.is_some());
        let web_meta = updated_issue.web_metadata.as_ref().unwrap();
        assert!(web_meta.github_url.is_some());
        assert!(web_meta.pull_request_url.is_some());
    }

    #[tokio::test]
    async fn test_get_statistics() {
        let (service, _agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;

        // Create issues with different statuses and priorities
        let issue1 = service.create_issue(
            "Open Issue".to_string(),
            "This is open".to_string(),
            IssuePriority::High,
            vec!["test".to_string()]
        ).await.expect("Failed to create issue");

        let issue2 = service.create_issue(
            "Critical Issue".to_string(),
            "This is critical".to_string(),
            IssuePriority::Critical,
            vec!["urgent".to_string()]
        ).await.expect("Failed to create issue");

        // Assign one issue
        let _assigned = service.assign_issue(issue1.id, agent_id).await.expect("Failed to assign issue");

        // Resolve one issue
        let _resolved = service.resolve_issue(issue2.id).await.expect("Failed to resolve issue");

        // Get statistics
        let stats = service.get_statistics().await.expect("Failed to get statistics");

        assert_eq!(stats.total_issues, 2);
        assert_eq!(stats.open_issues, 0); // issue1 was assigned (InProgress), issue2 was resolved
        assert_eq!(stats.in_progress_issues, 1);
        assert_eq!(stats.resolved_issues, 1);
        assert_eq!(stats.assigned_issues, 1);
        assert_eq!(stats.unassigned_issues, 1);
        assert_eq!(stats.issues_by_priority.get("High"), Some(&1));
        assert_eq!(stats.issues_by_priority.get("Critical"), Some(&1));
    }

    #[tokio::test]
    async fn test_get_issues_by_status() {
        let (service, _agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;

        // Create issues
        let issue1 = service.create_issue("Issue 1".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let issue2 = service.create_issue("Issue 2".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();

        // Assign one
        let _assigned = service.assign_issue(issue2.id, agent_id).await.unwrap();

        // Get open issues
        let open_issues = service.get_issues_by_status(&IssueStatus::Open).await.unwrap();
        assert_eq!(open_issues.len(), 1);
        assert_eq!(open_issues[0].id, issue1.id);

        // Get in-progress issues
        let in_progress_issues = service.get_issues_by_status(&IssueStatus::InProgress).await.unwrap();
        assert_eq!(in_progress_issues.len(), 1);
        assert_eq!(in_progress_issues[0].id, issue2.id);
    }

    #[tokio::test]
    async fn test_get_issues_by_priority() {
        let (service, _temp) = setup_test_service().await;

        // Create issues with different priorities
        let _low_issue = service.create_issue("Low Issue".to_string(), "Description".to_string(), IssuePriority::Low, vec![]).await.unwrap();
        let _high_issue = service.create_issue("High Issue".to_string(), "Description".to_string(), IssuePriority::High, vec![]).await.unwrap();
        let _critical_issue = service.create_issue("Critical Issue".to_string(), "Description".to_string(), IssuePriority::Critical, vec![]).await.unwrap();

        // Get high priority issues
        let high_issues = service.get_issues_by_priority(&IssuePriority::High).await.unwrap();
        assert_eq!(high_issues.len(), 1);
        assert_eq!(high_issues[0].title, "High Issue");

        // Get critical issues
        let critical_issues = service.get_issues_by_priority(&IssuePriority::Critical).await.unwrap();
        assert_eq!(critical_issues.len(), 1);
        assert_eq!(critical_issues[0].title, "Critical Issue");
    }

    #[tokio::test]
    async fn test_get_agent_issues() {
        let (service, agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;
        
        // Create a second agent
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8081")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent2 = Agent::builder()
            .name("test-agent-2")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();
        
        agent_repo.create(&agent2).await.expect("Failed to create second test agent");
        let other_agent_id = agent2.id;

        // Create and assign issues to different agents
        let issue1 = service.create_issue("Issue 1".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let issue2 = service.create_issue("Issue 2".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let _issue3 = service.create_issue("Issue 3".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap(); // Unassigned

        let _assigned1 = service.assign_issue(issue1.id, agent_id).await.unwrap();
        let _assigned2 = service.assign_issue(issue2.id, other_agent_id).await.unwrap();

        // Get issues for specific agent
        let agent_issues = service.get_agent_issues(agent_id).await.unwrap();
        assert_eq!(agent_issues.len(), 1);
        assert_eq!(agent_issues[0].id, issue1.id);

        let other_agent_issues = service.get_agent_issues(other_agent_id).await.unwrap();
        assert_eq!(other_agent_issues.len(), 1);
        assert_eq!(other_agent_issues[0].id, issue2.id);
    }

    #[tokio::test]
    async fn test_get_available_issues() {
        let (service, _agent_repo, agent_id, _temp) = setup_test_service_with_agent().await;

        // Create issues
        let issue1 = service.create_issue("Available Issue".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let issue2 = service.create_issue("To Be Assigned".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();

        // Assign one issue
        let _assigned = service.assign_issue(issue2.id, agent_id).await.unwrap();

        // Get available issues
        let available = service.get_available_issues().await.unwrap();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, issue1.id);
    }

    #[tokio::test]
    async fn test_get_priority_issues() {
        let (service, _temp) = setup_test_service().await;

        // Create issues with different priorities
        let _low_issue = service.create_issue("Low Issue".to_string(), "Description".to_string(), IssuePriority::Low, vec![]).await.unwrap();
        let _medium_issue = service.create_issue("Medium Issue".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let _high_issue = service.create_issue("High Issue".to_string(), "Description".to_string(), IssuePriority::High, vec![]).await.unwrap();
        let _critical_issue = service.create_issue("Critical Issue".to_string(), "Description".to_string(), IssuePriority::Critical, vec![]).await.unwrap();

        // Get priority issues (High and Critical only)
        let priority_issues = service.get_priority_issues().await.unwrap();
        assert_eq!(priority_issues.len(), 2);
        
        // Should be sorted with Critical first
        assert_eq!(priority_issues[0].title, "Critical Issue");
        assert_eq!(priority_issues[1].title, "High Issue");
    }

    #[tokio::test]
    async fn test_list_issues() {
        let (service, _temp) = setup_test_service().await;

        // Initially empty
        let issues = service.list_issues().await.unwrap();
        assert_eq!(issues.len(), 0);

        // Create issues
        let _issue1 = service.create_issue("Issue 1".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();
        let _issue2 = service.create_issue("Issue 2".to_string(), "Description".to_string(), IssuePriority::High, vec![]).await.unwrap();

        // List issues
        let issues = service.list_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_issue() {
        let (service, _temp) = setup_test_service().await;

        // Create an issue
        let issue = service.create_issue("Test Issue".to_string(), "Description".to_string(), IssuePriority::Medium, vec![]).await.unwrap();

        // Verify it exists
        let retrieved = service.get_issue(issue.id).await.unwrap();
        assert!(retrieved.is_some());

        // Delete it
        service.delete_issue(issue.id).await.expect("Failed to delete issue");

        // Verify it's gone
        let retrieved = service.get_issue(issue.id).await.unwrap();
        assert!(retrieved.is_none());
    }
}