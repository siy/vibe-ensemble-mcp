/// Tests for agent repository
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::*;
    use crate::Error;
    use vibe_ensemble_core::agent::{Agent, AgentStatus, AgentType, ConnectionMetadata};
    use sqlx::SqlitePool;
    use uuid::Uuid;

    async fn setup_test_db() -> AgentRepository {
        let pool = SqlitePool::connect(":memory:").await.expect("Failed to connect to test database");
        
        // Run migrations using proper module
        crate::migrations::run_migrations(&pool).await.expect("Failed to run migrations");
        
        AgentRepository::new(pool)
    }

    fn create_test_agent() -> Agent {
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        Agent::builder()
            .name("test-agent")
            .agent_type(AgentType::Worker)
            .capability("testing")
            .capability("validation")
            .connection_metadata(metadata)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_agent_create_and_find() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();

        // Create agent
        repo.create(&agent).await.expect("Failed to create agent");

        // Find by ID
        let found = repo.find_by_id(agent.id).await.expect("Failed to find agent");
        assert!(found.is_some());
        
        let found_agent = found.unwrap();
        assert_eq!(found_agent.id, agent.id);
        assert_eq!(found_agent.name, agent.name);
        assert_eq!(found_agent.agent_type, agent.agent_type);
        assert_eq!(found_agent.capabilities, agent.capabilities);
    }

    #[tokio::test]
    async fn test_agent_find_by_name() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();

        repo.create(&agent).await.expect("Failed to create agent");

        let found = repo.find_by_name(&agent.name).await.expect("Failed to find agent by name");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, agent.id);

        // Test non-existent name
        let not_found = repo.find_by_name("non-existent").await.expect("Query should succeed");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_agent_update() {
        let repo = setup_test_db().await;
        let mut agent = create_test_agent();

        repo.create(&agent).await.expect("Failed to create agent");

        // Update agent
        agent.add_capability("new-capability".to_string()).unwrap();
        agent.set_status(AgentStatus::Busy);

        repo.update(&agent).await.expect("Failed to update agent");

        // Verify update
        let found = repo.find_by_id(agent.id).await.expect("Failed to find agent").unwrap();
        assert!(found.has_capability("new-capability"));
        assert_eq!(found.status, AgentStatus::Busy);
    }

    #[tokio::test]
    async fn test_agent_update_status() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();

        repo.create(&agent).await.expect("Failed to create agent");

        // Update status
        repo.update_status(agent.id, &AgentStatus::Offline).await.expect("Failed to update status");

        let found = repo.find_by_id(agent.id).await.expect("Failed to find agent").unwrap();
        assert_eq!(found.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn test_agent_update_last_seen() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();

        repo.create(&agent).await.expect("Failed to create agent");
        
        let initial_last_seen = agent.last_seen;
        
        // Wait a bit to ensure timestamp difference
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        repo.update_last_seen(agent.id).await.expect("Failed to update last seen");

        let found = repo.find_by_id(agent.id).await.expect("Failed to find agent").unwrap();
        assert!(found.last_seen > initial_last_seen);
    }

    #[tokio::test]
    async fn test_agent_delete() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();

        repo.create(&agent).await.expect("Failed to create agent");
        
        // Verify exists
        assert!(repo.exists(agent.id).await.expect("Failed to check existence"));
        
        // Delete
        repo.delete(agent.id).await.expect("Failed to delete agent");
        
        // Verify deleted
        assert!(!repo.exists(agent.id).await.expect("Failed to check existence"));
        let found = repo.find_by_id(agent.id).await.expect("Query should succeed");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_agent_list() {
        let repo = setup_test_db().await;
        
        // Create multiple agents
        let agent1 = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8081")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("agent-1")
                .agent_type(AgentType::Worker)
                .capability("testing")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        let agent2 = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8082")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("agent-2")
                .agent_type(AgentType::Coordinator)
                .capability("coordination")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        repo.create(&agent1).await.expect("Failed to create agent1");
        repo.create(&agent2).await.expect("Failed to create agent2");

        // List all agents
        let agents = repo.list().await.expect("Failed to list agents");
        assert_eq!(agents.len(), 2);

        // Test count
        let count = repo.count().await.expect("Failed to count agents");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_agent_list_by_status() {
        let repo = setup_test_db().await;
        let mut agent1 = create_test_agent();
        let agent2 = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8082")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("agent-2")
                .agent_type(AgentType::Worker)
                .capability("testing")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        repo.create(&agent1).await.expect("Failed to create agent1");
        repo.create(&agent2).await.expect("Failed to create agent2");

        // Transition agents to online first
        agent1.go_online().expect("Failed to transition agent1 to online");
        let mut agent2 = agent2;
        agent2.go_online().expect("Failed to transition agent2 to online");
        repo.update(&agent1).await.expect("Failed to update agent1");
        repo.update(&agent2).await.expect("Failed to update agent2");

        // Set one agent to busy
        agent1.go_busy().expect("Failed to transition agent1 to busy");
        repo.update(&agent1).await.expect("Failed to update agent1");

        // List by status
        let online_agents = repo.list_by_status(&AgentStatus::Online).await.expect("Failed to list online agents");
        assert_eq!(online_agents.len(), 1);
        assert_eq!(online_agents[0].id, agent2.id);

        let busy_agents = repo.list_by_status(&AgentStatus::Busy).await.expect("Failed to list busy agents");
        assert_eq!(busy_agents.len(), 1);
        assert_eq!(busy_agents[0].id, agent1.id);

        // Test count by status
        let online_count = repo.count_by_status(&AgentStatus::Online).await.expect("Failed to count online agents");
        assert_eq!(online_count, 1);
    }

    #[tokio::test]
    async fn test_agent_list_by_type() {
        let repo = setup_test_db().await;
        let worker_agent = create_test_agent();
        let coordinator_agent = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8082")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("coordinator-1")
                .agent_type(AgentType::Coordinator)
                .capability("coordination")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        repo.create(&worker_agent).await.expect("Failed to create worker agent");
        repo.create(&coordinator_agent).await.expect("Failed to create coordinator agent");

        // List by type
        let workers = repo.list_by_type(&AgentType::Worker).await.expect("Failed to list workers");
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].id, worker_agent.id);

        let coordinators = repo.list_by_type(&AgentType::Coordinator).await.expect("Failed to list coordinators");
        assert_eq!(coordinators.len(), 1);
        assert_eq!(coordinators[0].id, coordinator_agent.id);
    }

    #[tokio::test]
    async fn test_agent_find_by_capability() {
        let repo = setup_test_db().await;
        let agent1 = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8081")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("agent-1")
                .agent_type(AgentType::Worker)
                .capability("rust")
                .capability("testing")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        let agent2 = {
            let metadata = ConnectionMetadata::builder()
                .endpoint("https://localhost:8082")
                .protocol_version("1.0")
                .build()
                .unwrap();

            Agent::builder()
                .name("agent-2")
                .agent_type(AgentType::Worker)
                .capability("python")
                .capability("testing")
                .connection_metadata(metadata)
                .build()
                .unwrap()
        };

        repo.create(&agent1).await.expect("Failed to create agent1");
        repo.create(&agent2).await.expect("Failed to create agent2");

        // Find by capability
        let rust_agents = repo.find_by_capability("rust").await.expect("Failed to find rust agents");
        assert_eq!(rust_agents.len(), 1);
        assert_eq!(rust_agents[0].id, agent1.id);

        let testing_agents = repo.find_by_capability("testing").await.expect("Failed to find testing agents");
        assert_eq!(testing_agents.len(), 2);

        let nonexistent_agents = repo.find_by_capability("nonexistent").await.expect("Failed to find nonexistent agents");
        assert_eq!(nonexistent_agents.len(), 0);
    }

    #[tokio::test]
    async fn test_agent_not_found_errors() {
        let repo = setup_test_db().await;
        let fake_id = Uuid::new_v4();

        // Update non-existent agent
        let fake_agent = create_test_agent();
        let result = repo.update(&fake_agent).await;
        assert!(matches!(result, Err(Error::NotFound { .. })));

        // Update status of non-existent agent
        let result = repo.update_status(fake_id, &AgentStatus::Offline).await;
        assert!(matches!(result, Err(Error::NotFound { .. })));

        // Update last seen of non-existent agent
        let result = repo.update_last_seen(fake_id).await;
        assert!(matches!(result, Err(Error::NotFound { .. })));

        // Delete non-existent agent
        let result = repo.delete(fake_id).await;
        assert!(matches!(result, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_agent_exists() {
        let repo = setup_test_db().await;
        let agent = create_test_agent();
        let fake_id = Uuid::new_v4();

        // Should not exist initially
        assert!(!repo.exists(agent.id).await.expect("Failed to check existence"));
        assert!(!repo.exists(fake_id).await.expect("Failed to check existence"));

        // Create agent
        repo.create(&agent).await.expect("Failed to create agent");

        // Should exist now
        assert!(repo.exists(agent.id).await.expect("Failed to check existence"));
        assert!(!repo.exists(fake_id).await.expect("Failed to check existence"));
    }
}