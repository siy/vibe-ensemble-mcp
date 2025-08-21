/// Tests for message service
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::*;
    use crate::repositories::{AgentRepository, MessageRepository};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tokio::time::Duration;
    use uuid::Uuid;
    use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};
    use vibe_ensemble_core::message::{MessagePriority, MessageType};

    async fn setup_test_service() -> (MessageService, SqlitePool) {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        
        // Run migrations
        crate::migrations::run_migrations(&pool).await.unwrap();
        
        let repository = Arc::new(MessageRepository::new(pool.clone()));
        let service = MessageService::new(repository);
        (service, pool)
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
    async fn test_send_direct_message() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "Test direct message".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.recipient_id, Some(recipient_id));
        assert_eq!(message.content, "Test direct message");
        assert!(!message.is_broadcast());
        assert!(!message.is_delivered());
    }

    #[tokio::test]
    async fn test_send_broadcast_message() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        
        let message = service
            .send_broadcast(
                sender_id,
                "Test broadcast message".to_string(),
                MessageType::Broadcast,
                MessagePriority::High,
            )
            .await
            .unwrap();
        
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.recipient_id, None);
        assert_eq!(message.content, "Test broadcast message");
        assert!(message.is_broadcast());
        assert!(message.is_delivered()); // Broadcasts are immediately delivered
    }

    #[tokio::test]
    async fn test_mark_message_delivered() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "Test delivery".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        assert!(!message.is_delivered());
        
        let delivered_message = service.mark_delivered(message.id).await.unwrap();
        assert!(delivered_message.is_delivered());
        
        // Verify delivery status
        let status = service.get_delivery_status(message.id).await.unwrap();
        assert!(status.is_some());
        assert_eq!(status.unwrap().status, DeliveryStatusType::Delivered);
    }

    #[tokio::test]
    async fn test_mark_delivery_failed() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "Test failure".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        let error_msg = "Network timeout".to_string();
        service.mark_delivery_failed(message.id, error_msg.clone()).await.unwrap();
        
        // Verify delivery status
        let status = service.get_delivery_status(message.id).await.unwrap();
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.status, DeliveryStatusType::Failed);
        assert_eq!(status.error_message, Some(error_msg));
    }

    #[tokio::test]
    async fn test_message_subscription() {
        let (service, pool) = setup_test_service().await;
        
        let mut receiver = service.subscribe().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send message in background
        tokio::spawn({
            let service = Arc::new(service);
            async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                service
                    .send_message(
                        sender_id,
                        recipient_id,
                        "Test subscription".to_string(),
                        MessageType::Direct,
                        MessagePriority::Normal,
                    )
                    .await
                    .unwrap();
            }
        });
        
        // Wait for event
        let event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        
        assert_eq!(event.event_type, MessageEventType::Sent);
        assert_eq!(event.message.sender_id, sender_id);
        assert_eq!(event.message.recipient_id, Some(recipient_id));
    }

    #[tokio::test]
    async fn test_agent_subscription() {
        let (service, _pool) = setup_test_service().await;
        
        let agent_id = Uuid::new_v4();
        let _receiver = service.subscribe_for_agent(agent_id).await;
        
        assert_eq!(service.get_active_subscriber_count().await, 1);
        
        service.unsubscribe(agent_id).await.unwrap();
        assert_eq!(service.get_active_subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_messages_for_recipient() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send multiple messages
        for i in 0..3 {
            service
                .send_message(
                    sender_id,
                    recipient_id,
                    format!("Message {}", i),
                    MessageType::Direct,
                    MessagePriority::Normal,
                )
                .await
                .unwrap();
        }
        
        let messages = service.get_messages_for_recipient(recipient_id).await.unwrap();
        assert_eq!(messages.len(), 3);
        
        for message in messages {
            assert_eq!(message.recipient_id, Some(recipient_id));
        }
    }

    #[tokio::test]
    async fn test_get_messages_from_sender() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient1 = create_test_agent(&pool, "recipient1").await;
        let recipient2 = create_test_agent(&pool, "recipient2").await;
        
        // Send messages to different recipients
        service
            .send_message(
                sender_id,
                recipient1,
                "Message 1".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        service
            .send_message(
                sender_id,
                recipient2,
                "Message 2".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        let messages = service.get_messages_from_sender(sender_id).await.unwrap();
        assert_eq!(messages.len(), 2);
        
        for message in messages {
            assert_eq!(message.sender_id, sender_id);
        }
    }

    #[tokio::test]
    async fn test_get_broadcast_messages() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        
        // Send broadcast messages
        for i in 0..2 {
            service
                .send_broadcast(
                    sender_id,
                    format!("Broadcast {}", i),
                    MessageType::Broadcast,
                    MessagePriority::Normal,
                )
                .await
                .unwrap();
        }
        
        // Send a direct message
        let recipient_id = create_test_agent(&pool, "recipient").await;
        service
            .send_message(
                sender_id,
                recipient_id,
                "Direct message".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        let broadcasts = service.get_broadcast_messages().await.unwrap();
        assert_eq!(broadcasts.len(), 2);
        
        for message in broadcasts {
            assert!(message.is_broadcast());
        }
    }

    #[tokio::test]
    async fn test_get_messages_by_type() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send messages of different types
        service
            .send_message(
                sender_id,
                recipient_id,
                "Direct".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        service
            .send_message(
                sender_id,
                recipient_id,
                "Status".to_string(),
                MessageType::StatusUpdate,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        let direct_messages = service.get_messages_by_type(&MessageType::Direct).await.unwrap();
        assert_eq!(direct_messages.len(), 1);
        assert_eq!(direct_messages[0].message_type, MessageType::Direct);
        
        let status_messages = service.get_messages_by_type(&MessageType::StatusUpdate).await.unwrap();
        assert_eq!(status_messages.len(), 1);
        assert_eq!(status_messages[0].message_type, MessageType::StatusUpdate);
    }

    #[tokio::test]
    async fn test_message_deletion() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "To be deleted".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        // Verify message exists
        let found = service.get_message(message.id).await.unwrap();
        assert!(found.is_some());
        
        // Delete message
        service.delete_message(message.id).await.unwrap();
        
        // Verify message is gone
        let found = service.get_message(message.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_message_statistics() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send various messages
        service
            .send_message(
                sender_id,
                recipient_id,
                "Direct 1".to_string(),
                MessageType::Direct,
                MessagePriority::High,
            )
            .await
            .unwrap();
        
        service
            .send_message(
                sender_id,
                recipient_id,
                "Direct 2".to_string(),
                MessageType::Direct,
                MessagePriority::Low,
            )
            .await
            .unwrap();
        
        service
            .send_broadcast(
                sender_id,
                "Broadcast".to_string(),
                MessageType::Broadcast,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        let stats = service.get_statistics().await.unwrap();
        
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.broadcast_messages, 1);
        assert_eq!(stats.delivered_messages, 1); // Only broadcast is delivered
        assert_eq!(stats.undelivered_messages, 2);
        
        // Check message type counts
        assert!(stats.messages_by_type.contains_key("Direct"));
        assert!(stats.messages_by_type.contains_key("Broadcast"));
        
        // Check priority counts
        assert!(stats.messages_by_priority.contains_key("High"));
        assert!(stats.messages_by_priority.contains_key("Low"));
        assert!(stats.messages_by_priority.contains_key("Normal"));
    }

    #[tokio::test]
    async fn test_validate_message_content() {
        let (service, _pool) = setup_test_service().await;
        
        // Valid content
        assert!(service.validate_message_content("Valid message").is_ok());
        
        // Empty content
        assert!(service.validate_message_content("").is_err());
        assert!(service.validate_message_content("   ").is_err());
        
        // Too long content
        let long_content = "a".repeat(10001);
        assert!(service.validate_message_content(&long_content).is_err());
        
        // Suspicious content
        assert!(service.validate_message_content("Hello <script>alert('xss')</script>").is_err());
        assert!(service.validate_message_content("javascript:alert('xss')").is_err());
        assert!(service.validate_message_content("data:text/html,<script>alert('xss')</script>").is_err());
    }

    #[tokio::test]
    async fn test_cleanup_stale_confirmations() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send a message
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "Test cleanup".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        // Mark as delivered to create a confirmation
        service.mark_delivered(message.id).await.unwrap();
        
        // Verify confirmation exists
        let status = service.get_delivery_status(message.id).await.unwrap();
        assert!(status.is_some());
        
        // Cleanup with very low threshold (should not remove recent confirmations)
        let cleaned = service.cleanup_stale_confirmations(24).await.unwrap();
        assert_eq!(cleaned, 0);
        
        // Verify confirmation still exists
        let status = service.get_delivery_status(message.id).await.unwrap();
        assert!(status.is_some());
        
        // Cleanup with zero threshold (should remove all confirmations)
        let cleaned = service.cleanup_stale_confirmations(0).await.unwrap();
        assert_eq!(cleaned, 1);
        
        // Verify confirmation is gone
        let status = service.get_delivery_status(message.id).await.unwrap();
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_get_recent_messages() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        // Send multiple messages
        for i in 0..5 {
            service
                .send_message(
                    sender_id,
                    recipient_id,
                    format!("Message {}", i),
                    MessageType::Direct,
                    MessagePriority::Normal,
                )
                .await
                .unwrap();
        }
        
        let recent = service.get_recent_messages(3).await.unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_double_delivery_marking() {
        let (service, pool) = setup_test_service().await;
        
        let sender_id = create_test_agent(&pool, "sender").await;
        let recipient_id = create_test_agent(&pool, "recipient").await;
        
        let message = service
            .send_message(
                sender_id,
                recipient_id,
                "Test double delivery".to_string(),
                MessageType::Direct,
                MessagePriority::Normal,
            )
            .await
            .unwrap();
        
        // Mark as delivered twice
        let delivered1 = service.mark_delivered(message.id).await.unwrap();
        let delivered2 = service.mark_delivered(message.id).await.unwrap();
        
        assert!(delivered1.is_delivered());
        assert!(delivered2.is_delivered());
        assert_eq!(delivered1.id, delivered2.id);
    }
}