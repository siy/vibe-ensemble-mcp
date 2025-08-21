/// Tests for message repository
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::*;
    use crate::Error;
    use crate::repositories::AgentRepository;
    use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};
    use vibe_ensemble_core::message::{Message, MessagePriority, MessageType};
    use sqlx::SqlitePool;
    use uuid::Uuid;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        
        // Run migrations using the proper migrations module
        crate::migrations::run_migrations(&pool).await.unwrap();
        
        pool
    }

    async fn create_test_agent(pool: &SqlitePool, name: &str) -> Uuid {
        let agent_repo = AgentRepository::new(pool.clone());
        
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent = Agent::builder()
            .name(name)
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        agent_repo.create(&agent).await.unwrap();
        agent.id
    }

    #[tokio::test]
    async fn test_message_create_and_find() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test message")
            .priority(MessagePriority::High)
            .message_type(MessageType::Direct)
            .build()
            .unwrap();

        // Create message
        repository.create(&message).await.unwrap();

        // Find message
        let found = repository.find_by_id(message.id).await.unwrap();
        assert!(found.is_some());
        
        let found_message = found.unwrap();
        assert_eq!(found_message.id, message.id);
        assert_eq!(found_message.sender_id, message.sender_id);
        assert_eq!(found_message.recipient_id, message.recipient_id);
        assert_eq!(found_message.content, message.content);
        assert_eq!(found_message.message_type, message.message_type);
        assert_eq!(found_message.metadata.priority, message.metadata.priority);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;

        let message = Message::broadcast_builder()
            .sender_id(sender_id)
            .content("Broadcast test")
            .priority(MessagePriority::Urgent)
            .message_type(MessageType::StatusUpdate)
            .build()
            .unwrap();

        // Create broadcast message
        repository.create(&message).await.unwrap();

        // Find message
        let found = repository.find_by_id(message.id).await.unwrap();
        assert!(found.is_some());
        
        let found_message = found.unwrap();
        assert_eq!(found_message.recipient_id, None);
        assert!(found_message.is_broadcast());
    }

    #[tokio::test]
    async fn test_message_update() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        let mut message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test update")
            .build()
            .unwrap();

        // Create message
        repository.create(&message).await.unwrap();

        // Mark as delivered
        message.mark_delivered();

        // Update message
        repository.update(&message).await.unwrap();

        // Find updated message
        let found = repository.find_by_id(message.id).await.unwrap();
        assert!(found.is_some());
        assert!(found.unwrap().is_delivered());
    }

    #[tokio::test]
    async fn test_message_delete() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test delete")
            .build()
            .unwrap();

        // Create message
        repository.create(&message).await.unwrap();

        // Delete message
        repository.delete(message.id).await.unwrap();

        // Verify deletion
        let found = repository.find_by_id(message.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_list_for_recipient() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        // Create multiple messages for the same recipient
        for i in 0..3 {
            let message = Message::builder()
                .sender_id(sender_id)
                .recipient_id(recipient_id)
                .content(format!("Test message {}", i))
                .build()
                .unwrap();
            
            repository.create(&message).await.unwrap();
        }

        // List messages for recipient
        let messages = repository.list_for_recipient(recipient_id).await.unwrap();
        assert_eq!(messages.len(), 3);

        // Verify all messages are for the correct recipient
        for message in messages {
            assert_eq!(message.recipient_id, Some(recipient_id));
        }
    }

    #[tokio::test]
    async fn test_list_from_sender() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient1 = create_test_agent(&pool, "recipient1").await;
        let recipient2 = create_test_agent(&pool, "recipient2").await;

        // Create multiple messages from the same sender
        for i in 0..2 {
            let message = Message::builder()
                .sender_id(sender_id)
                .recipient_id(if i % 2 == 0 { recipient1 } else { recipient2 })
                .content(format!("Test message {}", i))
                .build()
                .unwrap();
            
            repository.create(&message).await.unwrap();
        }

        // List messages from sender
        let messages = repository.list_from_sender(sender_id).await.unwrap();
        assert_eq!(messages.len(), 2);

        // Verify all messages are from the correct sender
        for message in messages {
            assert_eq!(message.sender_id, sender_id);
        }
    }

    #[tokio::test]
    async fn test_list_broadcast_messages() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;

        // Create some broadcast messages
        for i in 0..2 {
            let message = Message::broadcast_builder()
                .sender_id(sender_id)
                .content(format!("Broadcast {}", i))
                .build()
                .unwrap();
            
            repository.create(&message).await.unwrap();
        }

        // Create a direct message
        let recipient_id = create_test_agent(&pool, "recipient").await;
        let direct_message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Direct message")
            .build()
            .unwrap();
        
        repository.create(&direct_message).await.unwrap();

        // List broadcast messages
        let broadcasts = repository.list_broadcast_messages().await.unwrap();
        assert_eq!(broadcasts.len(), 2);

        // Verify all are broadcast messages
        for message in broadcasts {
            assert!(message.is_broadcast());
        }
    }

    #[tokio::test]
    async fn test_find_by_type() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        // Create messages of different types
        let types = vec![
            MessageType::Direct,
            MessageType::StatusUpdate,
            MessageType::IssueNotification,
            MessageType::KnowledgeShare,
        ];

        for msg_type in &types {
            let message = Message::builder()
                .sender_id(sender_id)
                .recipient_id(recipient_id)
                .content("Test type filtering")
                .message_type(msg_type.clone())
                .build()
                .unwrap();
            
            repository.create(&message).await.unwrap();
        }

        // Test filtering by type
        for msg_type in &types {
            let messages = repository.find_by_type(msg_type).await.unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].message_type, *msg_type);
        }
    }

    #[tokio::test]
    async fn test_count_operations() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        // Initially should be zero
        assert_eq!(repository.count().await.unwrap(), 0);
        assert_eq!(repository.count_undelivered().await.unwrap(), 0);

        // Create some messages
        let mut message1 = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Message 1")
            .build()
            .unwrap();

        let message2 = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Message 2")
            .build()
            .unwrap();

        repository.create(&message1).await.unwrap();
        repository.create(&message2).await.unwrap();

        // Check counts
        assert_eq!(repository.count().await.unwrap(), 2);
        assert_eq!(repository.count_undelivered().await.unwrap(), 2);

        // Mark one as delivered
        message1.mark_delivered();
        repository.update(&message1).await.unwrap();

        // Check counts again
        assert_eq!(repository.count().await.unwrap(), 2);
        assert_eq!(repository.count_undelivered().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_exists() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test exists")
            .build()
            .unwrap();

        // Should not exist initially
        assert!(!repository.exists(message.id).await.unwrap());

        // Create message
        repository.create(&message).await.unwrap();

        // Should exist now
        assert!(repository.exists(message.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_recent() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        // Create multiple messages
        for i in 0..5 {
            let message = Message::builder()
                .sender_id(sender_id)
                .recipient_id(recipient_id)
                .content(format!("Message {}", i))
                .build()
                .unwrap();
            
            repository.create(&message).await.unwrap();
        }

        // Get recent messages with limit
        let recent = repository.list_recent(3).await.unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_update_nonexistent() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Nonexistent")
            .build()
            .unwrap();

        // Try to update non-existent message
        let result = repository.update(&message).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let nonexistent_id = Uuid::new_v4();

        // Try to delete non-existent message
        let result = repository.delete(nonexistent_id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_message_metadata_serialization() {
        let pool = setup_test_db().await;
        let repository = MessageRepository::new(pool.clone());

        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        let correlation_id = Uuid::new_v4();
        let issue_id = Uuid::new_v4();

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test metadata")
            .priority(MessagePriority::High)
            .correlation_id(correlation_id)
            .issue_id(issue_id)
            .knowledge_ref("pattern-123")
            .knowledge_context("Test context")
            .require_confirmation()
            .build()
            .unwrap();

        // Create message
        repository.create(&message).await.unwrap();

        // Find and verify metadata
        let found = repository.find_by_id(message.id).await.unwrap();
        assert!(found.is_some());
        
        let found_message = found.unwrap();
        assert_eq!(found_message.metadata.priority, MessagePriority::High);
        assert_eq!(found_message.metadata.correlation_id, Some(correlation_id));
        assert_eq!(found_message.metadata.issue_id, Some(issue_id));
        assert_eq!(found_message.metadata.knowledge_refs.len(), 1);
        assert_eq!(found_message.metadata.knowledge_refs[0], "pattern-123");
        assert!(found_message.metadata.knowledge_context.is_some());
        assert!(found_message.metadata.delivery_confirmation);
    }
}