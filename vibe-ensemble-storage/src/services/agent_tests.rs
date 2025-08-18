/// Tests for agent service
#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{repositories::AgentRepository, Error};
    use vibe_ensemble_core::agent::{AgentStatus, AgentType, ConnectionMetadata};
    use sqlx::SqlitePool;
    use tempfile::NamedTempFile;
    use std::sync::Arc;

    async fn setup_test_service() -> (Arc<AgentService>, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let database_url = format!("sqlite://{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
        
        let repository = Arc::new(AgentRepository::new(pool));
        let service = Arc::new(AgentService::new(repository));
        (service, temp_file)
    }

    fn create_test_connection_metadata() -> ConnectionMetadata {
        ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .session_id("test-session-123")
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string(), "validation".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        assert_eq!(agent.name, "test-worker");
        assert_eq!(agent.agent_type, AgentType::Worker);
        assert!(agent.has_capability("testing"));
        assert!(agent.has_capability("validation"));
        assert_eq!(agent.status, AgentStatus::Online);

        // Check that agent is in active sessions
        assert!(service.is_agent_connected(agent.id).await);
        let session = service.get_session(agent.id).await.unwrap();
        assert_eq!(session.session_id, "session-123");
    }

    #[tokio::test]
    async fn test_duplicate_agent_name_registration() {
        let (service, _temp) = setup_test_service().await;

        // Register first agent
        service.register_agent(
            "duplicate-name".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-1".to_string(),
        ).await.expect("Failed to register first agent");

        // Try to register second agent with same name
        let result = service.register_agent(
            "duplicate-name".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-2".to_string(),
        ).await;

        assert!(matches!(result, Err(Error::Conflict(_))));
    }

    #[tokio::test]
    async fn test_agent_deregistration() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        // Verify agent is connected
        assert!(service.is_agent_connected(agent.id).await);

        // Deregister agent
        service.deregister_agent(agent.id).await.expect("Failed to deregister agent");

        // Verify agent is no longer connected
        assert!(!service.is_agent_connected(agent.id).await);

        // Verify agent status is offline
        let found_agent = service.get_agent(agent.id).await.expect("Failed to get agent").unwrap();
        assert_eq!(found_agent.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn test_agent_heartbeat_update() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        let initial_last_seen = agent.last_seen;
        
        // Wait a bit to ensure timestamp difference
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update heartbeat
        service.update_heartbeat(agent.id).await.expect("Failed to update heartbeat");

        // Verify last seen was updated
        let found_agent = service.get_agent(agent.id).await.expect("Failed to get agent").unwrap();
        assert!(found_agent.last_seen > initial_last_seen);

        // Verify session heartbeat was updated
        let session = service.get_session(agent.id).await.unwrap();
        assert!(session.last_heartbeat > agent.created_at);
    }

    #[tokio::test]
    async fn test_agent_status_update() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        // Update status to busy
        service.update_agent_status(agent.id, AgentStatus::Busy).await.expect("Failed to update status");

        let found_agent = service.get_agent(agent.id).await.expect("Failed to get agent").unwrap();
        assert_eq!(found_agent.status, AgentStatus::Busy);

        // Update status to offline (should remove from active sessions)
        service.update_agent_status(agent.id, AgentStatus::Offline).await.expect("Failed to update status");

        assert!(!service.is_agent_connected(agent.id).await);
    }

    #[tokio::test]
    async fn test_agent_listing() {
        let (service, _temp) = setup_test_service().await;

        // Register multiple agents
        let worker = service.register_agent(
            "worker-1".to_string(),
            AgentType::Worker,
            vec!["rust".to_string(), "testing".to_string()],
            create_test_connection_metadata(),
            "session-1".to_string(),
        ).await.expect("Failed to register worker");

        let coordinator = service.register_agent(
            "coordinator-1".to_string(),
            AgentType::Coordinator,
            vec!["coordination".to_string()],
            create_test_connection_metadata(),
            "session-2".to_string(),
        ).await.expect("Failed to register coordinator");

        // Test list all agents
        let all_agents = service.list_agents().await.expect("Failed to list all agents");
        assert_eq!(all_agents.len(), 2);

        // Test list online agents
        let online_agents = service.list_online_agents().await.expect("Failed to list online agents");
        assert_eq!(online_agents.len(), 2);

        // Test list by type
        let workers = service.list_agents_by_type(&AgentType::Worker).await.expect("Failed to list workers");
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].id, worker.id);

        let coordinators = service.list_agents_by_type(&AgentType::Coordinator).await.expect("Failed to list coordinators");
        assert_eq!(coordinators.len(), 1);
        assert_eq!(coordinators[0].id, coordinator.id);

        // Test find by capability
        let rust_agents = service.find_agents_by_capability("rust").await.expect("Failed to find rust agents");
        assert_eq!(rust_agents.len(), 1);
        assert_eq!(rust_agents[0].id, worker.id);

        let testing_agents = service.find_available_agents_by_capability("testing").await.expect("Failed to find testing agents");
        assert_eq!(testing_agents.len(), 1);
        assert_eq!(testing_agents[0].id, worker.id);
    }

    #[tokio::test]
    async fn test_agent_statistics() {
        let (service, _temp) = setup_test_service().await;

        // Initial statistics
        let stats = service.get_statistics().await.expect("Failed to get statistics");
        assert_eq!(stats.total_agents, 0);
        assert_eq!(stats.online_agents, 0);
        assert_eq!(stats.active_sessions, 0);

        // Register agents
        let worker = service.register_agent(
            "worker-1".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-1".to_string(),
        ).await.expect("Failed to register worker");

        let coordinator = service.register_agent(
            "coordinator-1".to_string(),
            AgentType::Coordinator,
            vec!["coordination".to_string()],
            create_test_connection_metadata(),
            "session-2".to_string(),
        ).await.expect("Failed to register coordinator");

        // Check updated statistics
        let stats = service.get_statistics().await.expect("Failed to get statistics");
        assert_eq!(stats.total_agents, 2);
        assert_eq!(stats.online_agents, 2);
        assert_eq!(stats.coordinator_agents, 1);
        assert_eq!(stats.worker_agents, 1);
        assert_eq!(stats.active_sessions, 2);

        // Set one agent to busy
        service.update_agent_status(worker.id, AgentStatus::Busy).await.expect("Failed to update status");

        let stats = service.get_statistics().await.expect("Failed to get statistics");
        assert_eq!(stats.online_agents, 1);
        assert_eq!(stats.busy_agents, 1);

        // Set one agent to offline
        service.update_agent_status(coordinator.id, AgentStatus::Offline).await.expect("Failed to update status");

        let stats = service.get_statistics().await.expect("Failed to get statistics");
        assert_eq!(stats.online_agents, 0);
        assert_eq!(stats.busy_agents, 1);
        assert_eq!(stats.offline_agents, 1);
        assert_eq!(stats.active_sessions, 1); // Only busy agent should remain in sessions
    }

    #[tokio::test]
    async fn test_agent_health_check() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        // Agent should be healthy initially
        let unhealthy = service.check_agent_health(60).await.expect("Failed to check health");
        assert!(unhealthy.is_empty());

        // Wait a brief moment to ensure the agent becomes stale
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Check with very short idle time (agent should become unhealthy)
        let unhealthy = service.check_agent_health(0).await.expect("Failed to check health");
        assert_eq!(unhealthy.len(), 1);
        assert_eq!(unhealthy[0], agent.id);

        // Verify agent was marked offline
        let found_agent = service.get_agent(agent.id).await.expect("Failed to get agent").unwrap();
        assert_eq!(found_agent.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn test_stale_session_cleanup() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "test-worker".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        // Verify agent is connected
        assert!(service.is_agent_connected(agent.id).await);

        // Wait a brief moment to ensure the session becomes stale
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Cleanup with very short idle time
        let stale = service.cleanup_stale_sessions(0).await.expect("Failed to cleanup stale sessions");
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0], agent.id);

        // Verify agent is no longer connected and marked offline
        assert!(!service.is_agent_connected(agent.id).await);
        let found_agent = service.get_agent(agent.id).await.expect("Failed to get agent").unwrap();
        assert_eq!(found_agent.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn test_get_active_sessions() {
        let (service, _temp) = setup_test_service().await;

        // No sessions initially
        let sessions = service.get_active_sessions().await;
        assert!(sessions.is_empty());

        // Register agents
        let agent1 = service.register_agent(
            "agent-1".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-1".to_string(),
        ).await.expect("Failed to register agent1");

        let _agent2 = service.register_agent(
            "agent-2".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-2".to_string(),
        ).await.expect("Failed to register agent2");

        // Check sessions
        let sessions = service.get_active_sessions().await;
        assert_eq!(sessions.len(), 2);

        let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        assert!(session_ids.contains(&"session-1".to_string()));
        assert!(session_ids.contains(&"session-2".to_string()));

        // Deregister one agent
        service.deregister_agent(agent1.id).await.expect("Failed to deregister agent");

        let sessions = service.get_active_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-2");
    }

    #[tokio::test]
    async fn test_get_agent_by_name() {
        let (service, _temp) = setup_test_service().await;

        let agent = service.register_agent(
            "unique-name".to_string(),
            AgentType::Worker,
            vec!["testing".to_string()],
            create_test_connection_metadata(),
            "session-123".to_string(),
        ).await.expect("Failed to register agent");

        // Find by name
        let found = service.get_agent_by_name("unique-name").await.expect("Failed to find agent by name");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, agent.id);

        // Try to find non-existent name
        let not_found = service.get_agent_by_name("non-existent").await.expect("Query should succeed");
        assert!(not_found.is_none());
    }
}