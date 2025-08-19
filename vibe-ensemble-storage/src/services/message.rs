//! Message service implementation for real-time messaging

use crate::{repositories::MessageRepository, Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::message::{Message, MessagePriority, MessageType};

/// Real-time message event for subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    pub event_type: MessageEventType,
    pub message: Message,
    pub timestamp: DateTime<Utc>,
}

/// Type of message event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageEventType {
    Sent,
    Delivered,
    Failed,
}

/// Message delivery status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStatus {
    pub message_id: Uuid,
    pub status: DeliveryStatusType,
    pub timestamp: DateTime<Utc>,
    pub error_message: Option<String>,
}

/// Type of delivery status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatusType {
    Pending,
    Delivered,
    Failed,
}

/// Message statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStatistics {
    pub total_messages: i64,
    pub undelivered_messages: i64,
    pub delivered_messages: i64,
    pub broadcast_messages: i64,
    pub messages_by_type: HashMap<String, i64>,
    pub messages_by_priority: HashMap<String, i64>,
    pub average_delivery_time_seconds: Option<f64>,
}

/// Real-time messaging service
pub struct MessageService {
    repository: Arc<MessageRepository>,
    event_broadcaster: broadcast::Sender<MessageEvent>,
    subscribers: Arc<RwLock<HashMap<Uuid, broadcast::Receiver<MessageEvent>>>>,
    delivery_confirmations: Arc<RwLock<HashMap<Uuid, DeliveryStatus>>>,
}

impl MessageService {
    /// Create a new message service
    pub fn new(repository: Arc<MessageRepository>) -> Self {
        let (event_broadcaster, _) = broadcast::channel(1000);

        Self {
            repository,
            event_broadcaster,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            delivery_confirmations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send a direct message
    pub async fn send_message(
        &self,
        sender_id: Uuid,
        recipient_id: Uuid,
        content: String,
        message_type: MessageType,
        priority: MessagePriority,
    ) -> Result<Message> {
        debug!(
            "Sending message from {} to {}: {}",
            sender_id, recipient_id, content
        );

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content(content)
            .message_type(message_type)
            .priority(priority)
            .build()
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to build message: {}", e)))?;

        // Store message
        self.repository.create(&message).await?;

        // Update delivery status
        let delivery_status = DeliveryStatus {
            message_id: message.id,
            status: DeliveryStatusType::Pending,
            timestamp: Utc::now(),
            error_message: None,
        };

        {
            let mut confirmations = self.delivery_confirmations.write().await;
            confirmations.insert(message.id, delivery_status);
        }

        // Broadcast event
        let event = MessageEvent {
            event_type: MessageEventType::Sent,
            message: message.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to broadcast message event: {}", e);
        }

        info!("Successfully sent message: {}", message.id);
        Ok(message)
    }

    /// Send a broadcast message
    pub async fn send_broadcast(
        &self,
        sender_id: Uuid,
        content: String,
        message_type: MessageType,
        priority: MessagePriority,
    ) -> Result<Message> {
        debug!("Sending broadcast from {}: {}", sender_id, content);

        let message = Message::broadcast_builder()
            .sender_id(sender_id)
            .content(content)
            .message_type(message_type)
            .priority(priority)
            .build()
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to build broadcast: {}", e)))?;

        // Store message
        self.repository.create(&message).await?;

        // Broadcast is immediately considered delivered
        let mut delivered_message = message.clone();
        delivered_message.mark_delivered();
        self.repository.update(&delivered_message).await?;

        // Broadcast event
        let event = MessageEvent {
            event_type: MessageEventType::Delivered,
            message: delivered_message.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to broadcast message event: {}", e);
        }

        info!("Successfully sent broadcast: {}", delivered_message.id);
        Ok(delivered_message)
    }

    /// Mark a message as delivered
    pub async fn mark_delivered(&self, message_id: Uuid) -> Result<Message> {
        debug!("Marking message as delivered: {}", message_id);

        let mut message = self
            .repository
            .find_by_id(message_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Message".to_string(),
                id: message_id.to_string(),
            })?;

        if message.is_delivered() {
            return Ok(message);
        }

        message.mark_delivered();
        self.repository.update(&message).await?;

        // Update delivery status
        let delivery_status = DeliveryStatus {
            message_id,
            status: DeliveryStatusType::Delivered,
            timestamp: Utc::now(),
            error_message: None,
        };

        {
            let mut confirmations = self.delivery_confirmations.write().await;
            confirmations.insert(message_id, delivery_status);
        }

        // Broadcast event
        let event = MessageEvent {
            event_type: MessageEventType::Delivered,
            message: message.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to broadcast delivery event: {}", e);
        }

        info!("Successfully marked message as delivered: {}", message_id);
        Ok(message)
    }

    /// Mark a message delivery as failed
    pub async fn mark_delivery_failed(&self, message_id: Uuid, error: String) -> Result<()> {
        debug!(
            "Marking message delivery as failed: {} - {}",
            message_id, error
        );

        let message = self
            .repository
            .find_by_id(message_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Message".to_string(),
                id: message_id.to_string(),
            })?;

        // Update delivery status
        let delivery_status = DeliveryStatus {
            message_id,
            status: DeliveryStatusType::Failed,
            timestamp: Utc::now(),
            error_message: Some(error.clone()),
        };

        {
            let mut confirmations = self.delivery_confirmations.write().await;
            confirmations.insert(message_id, delivery_status);
        }

        // Broadcast event
        let event = MessageEvent {
            event_type: MessageEventType::Failed,
            message,
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to broadcast failure event: {}", e);
        }

        warn!("Message delivery failed: {} - {}", message_id, error);
        Ok(())
    }

    /// Subscribe to real-time message events
    pub async fn subscribe(&self) -> broadcast::Receiver<MessageEvent> {
        debug!("Creating message event subscription");
        self.event_broadcaster.subscribe()
    }

    /// Subscribe to messages for a specific agent
    pub async fn subscribe_for_agent(&self, agent_id: Uuid) -> broadcast::Receiver<MessageEvent> {
        debug!("Creating message subscription for agent: {}", agent_id);
        let receiver = self.event_broadcaster.subscribe();

        // Store subscriber for management
        {
            let mut subscribers = self.subscribers.write().await;
            subscribers.insert(agent_id, self.event_broadcaster.subscribe());
        }

        receiver
    }

    /// Unsubscribe an agent from message events
    pub async fn unsubscribe(&self, agent_id: Uuid) -> Result<()> {
        debug!("Unsubscribing agent from message events: {}", agent_id);

        let mut subscribers = self.subscribers.write().await;
        if subscribers.remove(&agent_id).is_some() {
            info!("Successfully unsubscribed agent: {}", agent_id);
        } else {
            warn!("Agent was not subscribed: {}", agent_id);
        }

        Ok(())
    }

    /// Get messages for a recipient
    pub async fn get_messages_for_recipient(&self, recipient_id: Uuid) -> Result<Vec<Message>> {
        debug!("Getting messages for recipient: {}", recipient_id);
        self.repository.list_for_recipient(recipient_id).await
    }

    /// Get messages from a sender
    pub async fn get_messages_from_sender(&self, sender_id: Uuid) -> Result<Vec<Message>> {
        debug!("Getting messages from sender: {}", sender_id);
        self.repository.list_from_sender(sender_id).await
    }

    /// Get broadcast messages
    pub async fn get_broadcast_messages(&self) -> Result<Vec<Message>> {
        debug!("Getting broadcast messages");
        self.repository.list_broadcast_messages().await
    }

    /// Get recent messages
    pub async fn get_recent_messages(&self, limit: i64) -> Result<Vec<Message>> {
        debug!("Getting {} recent messages", limit);
        self.repository.list_recent(limit).await
    }

    /// Get messages within a specific time period
    pub async fn list_recent_messages(&self, duration: chrono::Duration) -> Result<Vec<Message>> {
        debug!("Getting messages from last {:?}", duration);
        let since = Utc::now() - duration;
        self.repository.list_since(since).await
    }

    /// Get messages by type
    pub async fn get_messages_by_type(&self, message_type: &MessageType) -> Result<Vec<Message>> {
        debug!("Getting messages by type: {:?}", message_type);
        self.repository.find_by_type(message_type).await
    }

    /// Get a specific message
    pub async fn get_message(&self, message_id: Uuid) -> Result<Option<Message>> {
        debug!("Getting message: {}", message_id);
        self.repository.find_by_id(message_id).await
    }

    /// Delete a message
    pub async fn delete_message(&self, message_id: Uuid) -> Result<()> {
        debug!("Deleting message: {}", message_id);

        // Remove from delivery confirmations
        {
            let mut confirmations = self.delivery_confirmations.write().await;
            confirmations.remove(&message_id);
        }

        self.repository.delete(message_id).await
    }

    /// Get delivery status for a message
    pub async fn get_delivery_status(&self, message_id: Uuid) -> Result<Option<DeliveryStatus>> {
        debug!("Getting delivery status for message: {}", message_id);

        let confirmations = self.delivery_confirmations.read().await;
        Ok(confirmations.get(&message_id).cloned())
    }

    /// Get message statistics
    pub async fn get_statistics(&self) -> Result<MessageStatistics> {
        debug!("Getting message statistics");

        let total_messages = self.repository.count().await?;
        let undelivered_messages = self.repository.count_undelivered().await?;
        let delivered_messages = total_messages - undelivered_messages;

        // Get broadcast message count
        let broadcast_messages = self.repository.list_broadcast_messages().await?.len() as i64;

        // Get messages by type
        let mut messages_by_type = HashMap::new();
        let types = vec![
            MessageType::Direct,
            MessageType::Broadcast,
            MessageType::StatusUpdate,
            MessageType::IssueNotification,
            MessageType::KnowledgeShare,
        ];

        for msg_type in types {
            let count = self.repository.find_by_type(&msg_type).await?.len() as i64;
            messages_by_type.insert(format!("{:?}", msg_type), count);
        }

        // Get messages by priority (approximate based on recent messages)
        let mut messages_by_priority = HashMap::new();
        let recent_messages = self.repository.list_recent(1000).await?;

        for message in recent_messages {
            let priority_str = format!("{:?}", message.metadata.priority);
            let count = messages_by_priority.get(&priority_str).unwrap_or(&0) + 1;
            messages_by_priority.insert(priority_str, count);
        }

        // Calculate average delivery time (simplified calculation)
        let average_delivery_time_seconds = self.calculate_average_delivery_time().await?;

        Ok(MessageStatistics {
            total_messages,
            undelivered_messages,
            delivered_messages,
            broadcast_messages,
            messages_by_type,
            messages_by_priority,
            average_delivery_time_seconds,
        })
    }

    /// Calculate average delivery time for recent messages
    async fn calculate_average_delivery_time(&self) -> Result<Option<f64>> {
        let recent_messages = self.repository.list_recent(100).await?;
        let delivered_times: Vec<i64> = recent_messages
            .into_iter()
            .filter_map(|msg| msg.delivery_time())
            .collect();

        if delivered_times.is_empty() {
            Ok(None)
        } else {
            let sum: i64 = delivered_times.iter().sum();
            let average = sum as f64 / delivered_times.len() as f64;
            Ok(Some(average))
        }
    }

    /// Validate message content and metadata
    pub fn validate_message_content(&self, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(Error::InvalidOperation(
                "Message content cannot be empty".to_string(),
            ));
        }

        if content.len() > 10000 {
            return Err(Error::InvalidOperation(
                "Message content too long (max 10000 characters)".to_string(),
            ));
        }

        // Check for potentially malicious content patterns
        let suspicious_patterns = vec!["<script", "javascript:", "data:text/html", "vbscript:"];
        let content_lower = content.to_lowercase();

        for pattern in suspicious_patterns {
            if content_lower.contains(pattern) {
                return Err(Error::InvalidOperation(format!(
                    "Message content contains suspicious pattern: {}",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Get active subscriber count
    pub async fn get_active_subscriber_count(&self) -> usize {
        let subscribers = self.subscribers.read().await;
        subscribers.len()
    }

    /// Clean up stale delivery confirmations
    pub async fn cleanup_stale_confirmations(&self, max_age_hours: i64) -> Result<usize> {
        debug!(
            "Cleaning up stale delivery confirmations older than {} hours",
            max_age_hours
        );

        let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
        let mut confirmations = self.delivery_confirmations.write().await;

        let initial_count = confirmations.len();
        confirmations.retain(|_, status| status.timestamp > cutoff_time);
        let cleaned_count = initial_count - confirmations.len();

        if cleaned_count > 0 {
            info!("Cleaned up {} stale delivery confirmations", cleaned_count);
        }

        Ok(cleaned_count)
    }
}

#[cfg(test)]
mod tests {
    include!("message_tests.rs");
}
