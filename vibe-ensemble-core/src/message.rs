//! Message domain model and related types
//!
//! This module provides the core message model for inter-agent communication
//! in the Vibe Ensemble system. Messages enable coordination and information
//! sharing between agents.
//!
//! # Examples
//!
//! Creating a direct message:
//!
//! ```rust
//! use vibe_ensemble_core::message::*;
//! use uuid::Uuid;
//!
//! let sender_id = Uuid::new_v4();
//! let recipient_id = Uuid::new_v4();
//!
//! let message = Message::builder()
//!     .sender_id(sender_id)
//!     .recipient_id(recipient_id)
//!     .content("Task assignment: Please review PR #123")
//!     .priority(MessagePriority::High)
//!     .message_type(MessageType::IssueNotification)
//!     .build()
//!     .unwrap();
//! ```
//!
//! Creating a broadcast message:
//!
//! ```rust
//! use vibe_ensemble_core::message::*;
//! use uuid::Uuid;
//!
//! let message = Message::broadcast_builder()
//!     .sender_id(Uuid::new_v4())
//!     .content("System maintenance scheduled for 2 AM UTC")
//!     .priority(MessagePriority::Normal)
//!     .build()
//!     .unwrap();
//! ```

use crate::{Error, Result};
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
    pub delivery_confirmation: bool,
    pub knowledge_context: Option<String>,
    pub is_compressed: bool,
    pub compression_type: Option<String>,
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
    /// Create a new direct message with validation
    pub fn new_direct(
        sender_id: Uuid,
        recipient_id: Uuid,
        content: String,
        priority: MessagePriority,
    ) -> Result<Self> {
        Self::validate_content(&content)?;

        Ok(Self {
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
                delivery_confirmation: false,
                knowledge_context: None,
                is_compressed: false,
                compression_type: None,
            },
            created_at: Utc::now(),
            delivered_at: None,
        })
    }

    /// Create a builder for constructing a Message
    pub fn builder() -> MessageBuilder {
        MessageBuilder::new()
    }

    /// Create a builder for constructing a broadcast Message
    pub fn broadcast_builder() -> BroadcastMessageBuilder {
        BroadcastMessageBuilder::new()
    }

    /// Validate message content
    fn validate_content(content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(Error::Validation {
                message: "Message content cannot be empty".to_string(),
            });
        }
        if content.len() > 10000 {
            return Err(Error::Validation {
                message: "Message content cannot exceed 10000 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Create a new broadcast message with validation
    pub fn new_broadcast(
        sender_id: Uuid,
        content: String,
        priority: MessagePriority,
    ) -> Result<Self> {
        Self::validate_content(&content)?;

        Ok(Self {
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
                delivery_confirmation: false,
                knowledge_context: None,
                is_compressed: false,
                compression_type: None,
            },
            created_at: Utc::now(),
            delivered_at: None,
        })
    }

    /// Mark the message as delivered
    pub fn mark_delivered(&mut self) {
        self.delivered_at = Some(Utc::now());
    }

    /// Check if the message has been delivered
    pub fn is_delivered(&self) -> bool {
        self.delivered_at.is_some()
    }

    /// Set the correlation ID for message threading
    pub fn set_correlation_id(&mut self, correlation_id: Uuid) {
        self.metadata.correlation_id = Some(correlation_id);
    }

    /// Associate the message with an issue
    pub fn set_issue_id(&mut self, issue_id: Uuid) {
        self.metadata.issue_id = Some(issue_id);
    }

    /// Add a knowledge reference to the message
    pub fn add_knowledge_ref(&mut self, knowledge_ref: String) -> Result<()> {
        if knowledge_ref.trim().is_empty() {
            return Err(Error::Validation {
                message: "Knowledge reference cannot be empty".to_string(),
            });
        }
        if !self.metadata.knowledge_refs.contains(&knowledge_ref) {
            self.metadata.knowledge_refs.push(knowledge_ref);
        }
        Ok(())
    }

    /// Set the knowledge context for the message
    pub fn set_knowledge_context(&mut self, context: String) -> Result<()> {
        if context.trim().is_empty() {
            return Err(Error::Validation {
                message: "Knowledge context cannot be empty".to_string(),
            });
        }
        self.metadata.knowledge_context = Some(context);
        Ok(())
    }

    /// Enable delivery confirmation for the message
    pub fn require_delivery_confirmation(&mut self) {
        self.metadata.delivery_confirmation = true;
    }

    /// Check if the message is a broadcast
    pub fn is_broadcast(&self) -> bool {
        self.recipient_id.is_none()
    }

    /// Get the age of the message in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds()
    }

    /// Check if the message requires delivery confirmation
    pub fn requires_confirmation(&self) -> bool {
        self.metadata.delivery_confirmation
    }

    /// Get the delivery time if delivered
    pub fn delivery_time(&self) -> Option<i64> {
        self.delivered_at.map(|delivered| {
            delivered
                .signed_duration_since(self.created_at)
                .num_seconds()
        })
    }
}

impl MessageMetadata {
    /// Create new message metadata
    pub fn new(priority: MessagePriority) -> Self {
        Self {
            correlation_id: None,
            issue_id: None,
            knowledge_refs: Vec::new(),
            priority,
            delivery_confirmation: false,
            knowledge_context: None,
            is_compressed: false,
            compression_type: None,
        }
    }

    /// Create a builder for metadata
    pub fn builder() -> MessageMetadataBuilder {
        MessageMetadataBuilder::new()
    }
}

/// Builder for constructing MessageMetadata instances
#[derive(Debug, Clone)]
pub struct MessageMetadataBuilder {
    correlation_id: Option<Uuid>,
    issue_id: Option<Uuid>,
    knowledge_refs: Vec<String>,
    priority: MessagePriority,
    delivery_confirmation: bool,
    knowledge_context: Option<String>,
    is_compressed: bool,
    compression_type: Option<String>,
}

impl MessageMetadataBuilder {
    /// Create a new metadata builder
    pub fn new() -> Self {
        Self {
            correlation_id: None,
            issue_id: None,
            knowledge_refs: Vec::new(),
            priority: MessagePriority::Normal,
            delivery_confirmation: false,
            knowledge_context: None,
            is_compressed: false,
            compression_type: None,
        }
    }

    /// Set the priority
    pub fn priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the correlation ID
    pub fn correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set the issue ID
    pub fn issue_id(mut self, id: Uuid) -> Self {
        self.issue_id = Some(id);
        self
    }

    /// Add a knowledge reference
    pub fn knowledge_ref<S: Into<String>>(mut self, reference: S) -> Self {
        self.knowledge_refs.push(reference.into());
        self
    }

    /// Set delivery confirmation requirement
    pub fn require_confirmation(mut self) -> Self {
        self.delivery_confirmation = true;
        self
    }

    /// Set knowledge context
    pub fn knowledge_context<S: Into<String>>(mut self, context: S) -> Self {
        self.knowledge_context = Some(context.into());
        self
    }

    /// Build the metadata
    pub fn build(self) -> MessageMetadata {
        MessageMetadata {
            correlation_id: self.correlation_id,
            issue_id: self.issue_id,
            knowledge_refs: self.knowledge_refs,
            priority: self.priority,
            delivery_confirmation: self.delivery_confirmation,
            knowledge_context: self.knowledge_context,
            is_compressed: self.is_compressed,
            compression_type: self.compression_type,
        }
    }
}

impl Default for MessageMetadataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing Message instances with validation
#[derive(Debug, Clone)]
pub struct MessageBuilder {
    sender_id: Option<Uuid>,
    recipient_id: Option<Uuid>,
    message_type: Option<MessageType>,
    content: Option<String>,
    metadata: MessageMetadataBuilder,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self {
            sender_id: None,
            recipient_id: None,
            message_type: None,
            content: None,
            metadata: MessageMetadataBuilder::new(),
        }
    }

    /// Set the sender ID
    pub fn sender_id(mut self, id: Uuid) -> Self {
        self.sender_id = Some(id);
        self
    }

    /// Set the recipient ID
    pub fn recipient_id(mut self, id: Uuid) -> Self {
        self.recipient_id = Some(id);
        self
    }

    /// Set the message type
    pub fn message_type(mut self, msg_type: MessageType) -> Self {
        self.message_type = Some(msg_type);
        self
    }

    /// Set the message content
    pub fn content<S: Into<String>>(mut self, content: S) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the message priority
    pub fn priority(mut self, priority: MessagePriority) -> Self {
        self.metadata = self.metadata.priority(priority);
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.correlation_id(id);
        self
    }

    /// Set issue ID
    pub fn issue_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.issue_id(id);
        self
    }

    /// Add knowledge reference
    pub fn knowledge_ref<S: Into<String>>(mut self, reference: S) -> Self {
        self.metadata = self.metadata.knowledge_ref(reference);
        self
    }

    /// Require delivery confirmation
    pub fn require_confirmation(mut self) -> Self {
        self.metadata = self.metadata.require_confirmation();
        self
    }

    /// Set knowledge context
    pub fn knowledge_context<S: Into<String>>(mut self, context: S) -> Self {
        self.metadata = self.metadata.knowledge_context(context);
        self
    }

    /// Build the Message instance
    pub fn build(self) -> Result<Message> {
        let sender_id = self.sender_id.ok_or_else(|| Error::Validation {
            message: "Sender ID is required".to_string(),
        })?;
        let recipient_id = self.recipient_id.ok_or_else(|| Error::Validation {
            message: "Recipient ID is required for direct messages".to_string(),
        })?;
        let message_type = self.message_type.unwrap_or(MessageType::Direct);
        let content = self.content.ok_or_else(|| Error::Validation {
            message: "Message content is required".to_string(),
        })?;

        Message::validate_content(&content)?;

        Ok(Message {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id: Some(recipient_id),
            message_type,
            content,
            metadata: self.metadata.build(),
            created_at: Utc::now(),
            delivered_at: None,
        })
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing broadcast Message instances
#[derive(Debug, Clone)]
pub struct BroadcastMessageBuilder {
    sender_id: Option<Uuid>,
    message_type: Option<MessageType>,
    content: Option<String>,
    metadata: MessageMetadataBuilder,
}

impl BroadcastMessageBuilder {
    /// Create a new broadcast message builder
    pub fn new() -> Self {
        Self {
            sender_id: None,
            message_type: None,
            content: None,
            metadata: MessageMetadataBuilder::new(),
        }
    }

    /// Set the sender ID
    pub fn sender_id(mut self, id: Uuid) -> Self {
        self.sender_id = Some(id);
        self
    }

    /// Set the message type
    pub fn message_type(mut self, msg_type: MessageType) -> Self {
        self.message_type = Some(msg_type);
        self
    }

    /// Set the message content
    pub fn content<S: Into<String>>(mut self, content: S) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the message priority
    pub fn priority(mut self, priority: MessagePriority) -> Self {
        self.metadata = self.metadata.priority(priority);
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.correlation_id(id);
        self
    }

    /// Set issue ID
    pub fn issue_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.issue_id(id);
        self
    }

    /// Add knowledge reference
    pub fn knowledge_ref<S: Into<String>>(mut self, reference: S) -> Self {
        self.metadata = self.metadata.knowledge_ref(reference);
        self
    }

    /// Set knowledge context
    pub fn knowledge_context<S: Into<String>>(mut self, context: S) -> Self {
        self.metadata = self.metadata.knowledge_context(context);
        self
    }

    /// Build the broadcast Message instance
    pub fn build(self) -> Result<Message> {
        let sender_id = self.sender_id.ok_or_else(|| Error::Validation {
            message: "Sender ID is required".to_string(),
        })?;
        let message_type = self.message_type.unwrap_or(MessageType::Broadcast);
        let content = self.content.ok_or_else(|| Error::Validation {
            message: "Message content is required".to_string(),
        })?;

        Message::validate_content(&content)?;

        Ok(Message {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id: None,
            message_type,
            content,
            metadata: self.metadata.build(),
            created_at: Utc::now(),
            delivered_at: None,
        })
    }
}

impl Default for BroadcastMessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_message_creation() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test message")
            .priority(MessagePriority::High)
            .message_type(MessageType::Direct)
            .build()
            .unwrap();

        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.recipient_id, Some(recipient_id));
        assert_eq!(message.content, "Test message");
        assert_eq!(message.metadata.priority, MessagePriority::High);
        assert_eq!(message.message_type, MessageType::Direct);
        assert!(!message.is_broadcast());
        assert!(!message.is_delivered());
    }

    #[test]
    fn test_broadcast_message_creation() {
        let sender_id = Uuid::new_v4();

        let message = Message::broadcast_builder()
            .sender_id(sender_id)
            .content("Broadcast announcement")
            .priority(MessagePriority::Urgent)
            .message_type(MessageType::StatusUpdate)
            .build()
            .unwrap();

        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.recipient_id, None);
        assert_eq!(message.content, "Broadcast announcement");
        assert_eq!(message.metadata.priority, MessagePriority::Urgent);
        assert_eq!(message.message_type, MessageType::StatusUpdate);
        assert!(message.is_broadcast());
    }

    #[test]
    fn test_message_content_validation() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();

        // Empty content should fail
        let result = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("")
            .build();
        assert!(result.is_err());

        // Too long content should fail
        let long_content = "a".repeat(10001);
        let result = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content(long_content)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_message_delivery() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();

        let mut message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test delivery")
            .build()
            .unwrap();

        assert!(!message.is_delivered());
        assert!(message.delivery_time().is_none());

        message.mark_delivered();
        assert!(message.is_delivered());
        assert!(message.delivery_time().is_some());
        assert!(message.delivery_time().unwrap() >= 0);
    }

    #[test]
    fn test_message_metadata_operations() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let issue_id = Uuid::new_v4();

        let mut message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test metadata")
            .correlation_id(correlation_id)
            .issue_id(issue_id)
            .knowledge_ref("pattern-001")
            .knowledge_context("Testing context")
            .require_confirmation()
            .build()
            .unwrap();

        assert_eq!(message.metadata.correlation_id, Some(correlation_id));
        assert_eq!(message.metadata.issue_id, Some(issue_id));
        assert_eq!(message.metadata.knowledge_refs.len(), 1);
        assert!(message.metadata.knowledge_context.is_some());
        assert!(message.requires_confirmation());

        // Test adding knowledge references
        message
            .add_knowledge_ref("pattern-002".to_string())
            .unwrap();
        assert_eq!(message.metadata.knowledge_refs.len(), 2);

        // Test adding empty knowledge reference
        let result = message.add_knowledge_ref("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_message_age() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();

        let message = Message::builder()
            .sender_id(sender_id)
            .recipient_id(recipient_id)
            .content("Test age")
            .build()
            .unwrap();

        let age = message.age_seconds();
        assert!(age >= 0);
        assert!(age < 60); // Should be very recent
    }

    #[test]
    fn test_message_builder_validation() {
        // Missing sender should fail
        let result = Message::builder()
            .recipient_id(Uuid::new_v4())
            .content("Test")
            .build();
        assert!(result.is_err());

        // Missing recipient for direct message should fail
        let result = Message::builder()
            .sender_id(Uuid::new_v4())
            .content("Test")
            .build();
        assert!(result.is_err());

        // Missing content should fail
        let result = Message::builder()
            .sender_id(Uuid::new_v4())
            .recipient_id(Uuid::new_v4())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_broadcast_builder_validation() {
        // Missing sender should fail
        let result = Message::broadcast_builder()
            .content("Broadcast test")
            .build();
        assert!(result.is_err());

        // Missing content should fail
        let result = Message::broadcast_builder()
            .sender_id(Uuid::new_v4())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_legacy_constructor_methods() {
        let sender_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();

        // Test direct message legacy constructor
        let message = Message::new_direct(
            sender_id,
            recipient_id,
            "Direct test".to_string(),
            MessagePriority::Normal,
        )
        .unwrap();
        assert!(!message.is_broadcast());

        // Test broadcast message legacy constructor
        let message = Message::new_broadcast(
            sender_id,
            "Broadcast test".to_string(),
            MessagePriority::Low,
        )
        .unwrap();
        assert!(message.is_broadcast());
    }
}
