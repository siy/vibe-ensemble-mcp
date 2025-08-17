//! Message domain model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a message between agents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Option<Uuid>, // None for broadcast messages
    pub message_type: MessageType,
    pub content: String,
    pub metadata: MessageMetadata,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

/// Type of message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Direct,
    Broadcast,
    StatusUpdate,
    IssueNotification,
    KnowledgeShare,
}

/// Additional metadata for messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageMetadata {
    pub correlation_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub knowledge_refs: Vec<String>,
    pub priority: MessagePriority,
}

/// Priority of a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Message {
    /// Create a new direct message
    pub fn new_direct(
        sender_id: Uuid,
        recipient_id: Uuid,
        content: String,
        priority: MessagePriority,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id: Some(recipient_id),
            message_type: MessageType::Direct,
            content,
            metadata: MessageMetadata {
                correlation_id: None,
                issue_id: None,
                knowledge_refs: Vec::new(),
                priority,
            },
            created_at: Utc::now(),
            delivered_at: None,
        }
    }

    /// Create a new broadcast message
    pub fn new_broadcast(sender_id: Uuid, content: String, priority: MessagePriority) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id: None,
            message_type: MessageType::Broadcast,
            content,
            metadata: MessageMetadata {
                correlation_id: None,
                issue_id: None,
                knowledge_refs: Vec::new(),
                priority,
            },
            created_at: Utc::now(),
            delivered_at: None,
        }
    }

    /// Mark the message as delivered
    pub fn mark_delivered(&mut self) {
        self.delivered_at = Some(Utc::now());
    }

    /// Check if the message has been delivered
    pub fn is_delivered(&self) -> bool {
        self.delivered_at.is_some()
    }
}