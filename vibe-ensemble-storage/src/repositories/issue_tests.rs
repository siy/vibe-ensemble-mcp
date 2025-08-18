/// Tests for issue repository
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::*;
    use crate::{Error, repositories::AgentRepository};
    use vibe_ensemble_core::{
        issue::{Issue, IssueStatus, IssuePriority},
        agent::{Agent, AgentType, ConnectionMetadata}
    };
    use sqlx::SqlitePool;
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    async fn setup_test_db() -> (IssueRepository, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let database_url = format!("sqlite://{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
        
        let repository = IssueRepository::new(pool);
        (repository, temp_file)
    }

    async fn setup_test_db_with_agent() -> (IssueRepository, AgentRepository, Uuid, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let database_url = format!("sqlite://{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
        
        let issue_repo = IssueRepository::new(pool.clone());
        let agent_repo = AgentRepository::new(pool);
        
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
        
        agent_repo.create(&agent).await.expect("Failed to create test agent");
        
        (issue_repo, agent_repo, agent.id, temp_file)
    }

    fn create_test_issue() -> Issue {
        Issue::builder()
            .title("Test Issue")
            .description("This is a test issue for repository validation")
            .priority(IssuePriority::Medium)
            .tag("test")
            .tag("repository")
            .build()
            .unwrap()
    }

    fn create_critical_issue() -> Issue {
        Issue::builder()
            .title("Critical Bug Fix")
            .description("This is a critical issue that needs immediate attention")
            .priority(IssuePriority::Critical)
            .tag("bug")
            .tag("critical")
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_issue_create_and_find() {
        let (repo, _temp) = setup_test_db().await;
        let issue = create_test_issue();

        // Create issue
        repo.create(&issue).await.expect("Failed to create issue");

        // Find by ID
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());
        
        let found_issue = found.unwrap();
        assert_eq!(found_issue.id, issue.id);
        assert_eq!(found_issue.title, issue.title);
        assert_eq!(found_issue.description, issue.description);
        assert_eq!(found_issue.status, issue.status);
        assert_eq!(found_issue.priority, issue.priority);
        assert_eq!(found_issue.tags, issue.tags);
    }

    #[tokio::test]
    async fn test_issue_not_found() {
        let (repo, _temp) = setup_test_db().await;
        let non_existent_id = Uuid::new_v4();

        let result = repo.find_by_id(non_existent_id).await.expect("Query should not fail");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_issue_update() {
        let (repo, _temp) = setup_test_db().await;
        let mut issue = create_test_issue();

        // Create issue
        repo.create(&issue).await.expect("Failed to create issue");

        // Update issue
        issue.set_priority(IssuePriority::High);
        issue.add_tag("updated".to_string()).unwrap();
        
        repo.update(&issue).await.expect("Failed to update issue");

        // Verify update
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());
        
        let found_issue = found.unwrap();
        assert_eq!(found_issue.priority, IssuePriority::High);
        assert!(found_issue.has_tag("updated"));
    }

    #[tokio::test]
    async fn test_issue_delete() {
        let (repo, _temp) = setup_test_db().await;
        let issue = create_test_issue();

        // Create issue
        repo.create(&issue).await.expect("Failed to create issue");

        // Verify it exists
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());

        // Delete issue
        repo.delete(issue.id).await.expect("Failed to delete issue");

        // Verify it's gone
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_issue_delete_nonexistent() {
        let (repo, _temp) = setup_test_db().await;
        let non_existent_id = Uuid::new_v4();

        let result = repo.delete(non_existent_id).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::NotFound { .. } => {}, // Expected
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_issue_list() {
        let (repo, _temp) = setup_test_db().await;
        let issue1 = create_test_issue();
        let issue2 = create_critical_issue();

        // Initially empty
        let issues = repo.list().await.expect("Failed to list issues");
        assert_eq!(issues.len(), 0);

        // Create issues
        repo.create(&issue1).await.expect("Failed to create issue1");
        repo.create(&issue2).await.expect("Failed to create issue2");

        // List issues
        let issues = repo.list().await.expect("Failed to list issues");
        assert_eq!(issues.len(), 2);
        
        // Should be ordered by created_at DESC (newest first)
        assert_eq!(issues[0].id, issue2.id); // Created second, should be first
        assert_eq!(issues[1].id, issue1.id);
    }

    #[tokio::test]
    async fn test_issue_count() {
        let (repo, _temp) = setup_test_db().await;

        // Initially empty
        let count = repo.count().await.expect("Failed to count issues");
        assert_eq!(count, 0);

        // Create issues
        let issue1 = create_test_issue();
        let issue2 = create_critical_issue();
        repo.create(&issue1).await.expect("Failed to create issue1");
        repo.create(&issue2).await.expect("Failed to create issue2");

        let count = repo.count().await.expect("Failed to count issues");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_issue_find_by_status() {
        let (repo, _agent_repo, agent_id, _temp) = setup_test_db_with_agent().await;
        let mut issue1 = create_test_issue();
        let issue2 = create_critical_issue();

        // Create issues
        repo.create(&issue1).await.expect("Failed to create issue1");
        repo.create(&issue2).await.expect("Failed to create issue2");

        // Update one to InProgress
        issue1.assign_to(agent_id);
        repo.update(&issue1).await.expect("Failed to update issue1");

        // Find by status
        let open_issues = repo.find_by_status(&IssueStatus::Open).await.expect("Failed to find open issues");
        assert_eq!(open_issues.len(), 1);
        assert_eq!(open_issues[0].id, issue2.id);

        let in_progress_issues = repo.find_by_status(&IssueStatus::InProgress).await.expect("Failed to find in-progress issues");
        assert_eq!(in_progress_issues.len(), 1);
        assert_eq!(in_progress_issues[0].id, issue1.id);
    }

    #[tokio::test]
    async fn test_issue_find_by_priority() {
        let (repo, _temp) = setup_test_db().await;
        let issue1 = create_test_issue(); // Medium priority
        let issue2 = create_critical_issue(); // Critical priority

        // Create issues
        repo.create(&issue1).await.expect("Failed to create issue1");
        repo.create(&issue2).await.expect("Failed to create issue2");

        // Find by priority
        let medium_issues = repo.find_by_priority(&IssuePriority::Medium).await.expect("Failed to find medium issues");
        assert_eq!(medium_issues.len(), 1);
        assert_eq!(medium_issues[0].id, issue1.id);

        let critical_issues = repo.find_by_priority(&IssuePriority::Critical).await.expect("Failed to find critical issues");
        assert_eq!(critical_issues.len(), 1);
        assert_eq!(critical_issues[0].id, issue2.id);

        let low_issues = repo.find_by_priority(&IssuePriority::Low).await.expect("Failed to find low issues");
        assert_eq!(low_issues.len(), 0);
    }

    #[tokio::test]
    async fn test_issue_find_by_assigned_agent() {
        let (repo, agent_repo, agent_id1, _temp) = setup_test_db_with_agent().await;
        let mut issue1 = create_test_issue();
        let mut issue2 = create_critical_issue();

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
        let agent_id2 = agent2.id;

        // Assign issues to different agents
        issue1.assign_to(agent_id1);
        issue2.assign_to(agent_id2);

        // Create issues
        repo.create(&issue1).await.expect("Failed to create issue1");
        repo.create(&issue2).await.expect("Failed to create issue2");

        // Find by assigned agent
        let agent1_issues = repo.find_by_assigned_agent(agent_id1).await.expect("Failed to find agent1 issues");
        assert_eq!(agent1_issues.len(), 1);
        assert_eq!(agent1_issues[0].id, issue1.id);

        let agent2_issues = repo.find_by_assigned_agent(agent_id2).await.expect("Failed to find agent2 issues");
        assert_eq!(agent2_issues.len(), 1);
        assert_eq!(agent2_issues[0].id, issue2.id);

        let unassigned_agent_id = Uuid::new_v4();
        let unassigned_issues = repo.find_by_assigned_agent(unassigned_agent_id).await.expect("Failed to find unassigned agent issues");
        assert_eq!(unassigned_issues.len(), 0);
    }

    #[tokio::test]
    async fn test_issue_exists() {
        let (repo, _temp) = setup_test_db().await;
        let issue = create_test_issue();

        // Should not exist initially
        let exists = repo.exists(issue.id).await.expect("Failed to check existence");
        assert!(!exists);

        // Create issue
        repo.create(&issue).await.expect("Failed to create issue");

        // Should exist now
        let exists = repo.exists(issue.id).await.expect("Failed to check existence");
        assert!(exists);

        // Delete issue
        repo.delete(issue.id).await.expect("Failed to delete issue");

        // Should not exist again
        let exists = repo.exists(issue.id).await.expect("Failed to check existence");
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_issue_status_serialization() {
        let (repo, _temp) = setup_test_db().await;
        let mut issue = create_test_issue();

        // Test blocked status
        issue.block("Waiting for dependencies".to_string()).unwrap();
        repo.create(&issue).await.expect("Failed to create blocked issue");

        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());
        
        let found_issue = found.unwrap();
        match &found_issue.status {
            IssueStatus::Blocked { reason } => {
                assert_eq!(reason, "Waiting for dependencies");
            },
            _ => panic!("Expected blocked status"),
        }
    }

    #[tokio::test]
    async fn test_issue_assignment_tracking() {
        let (repo, _agent_repo, agent_id, _temp) = setup_test_db_with_agent().await;
        let mut issue = create_test_issue();

        // Initially unassigned
        assert!(issue.assigned_agent_id.is_none());

        // Assign to agent
        issue.assign_to(agent_id);
        repo.create(&issue).await.expect("Failed to create assigned issue");

        // Verify assignment is persisted
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());
        
        let found_issue = found.unwrap();
        assert_eq!(found_issue.assigned_agent_id, Some(agent_id));
        assert_eq!(found_issue.status, IssueStatus::InProgress);
    }

    #[tokio::test]
    async fn test_issue_resolution_tracking() {
        let (repo, _temp) = setup_test_db().await;
        let mut issue = create_test_issue();

        // Initially not resolved
        assert!(issue.resolved_at.is_none());

        repo.create(&issue).await.expect("Failed to create issue");

        // Resolve issue
        issue.resolve();
        repo.update(&issue).await.expect("Failed to update resolved issue");

        // Verify resolution is persisted
        let found = repo.find_by_id(issue.id).await.expect("Failed to find issue");
        assert!(found.is_some());
        
        let found_issue = found.unwrap();
        assert_eq!(found_issue.status, IssueStatus::Resolved);
        assert!(found_issue.resolved_at.is_some());
    }

    #[tokio::test]
    async fn test_issue_update_nonexistent() {
        let (repo, _temp) = setup_test_db().await;
        let issue = create_test_issue();

        // Try to update non-existent issue
        let result = repo.update(&issue).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::NotFound { .. } => {}, // Expected
            _ => panic!("Expected NotFound error"),
        }
    }
}