//! Message repository implementation

use crate::{Error, Result};
use anyhow;
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::{Pool, Sqlite};
use tracing::{debug, info};
use uuid::Uuid;
use vibe_ensemble_core::message::{Message, MessageMetadata, MessageType};

/// Repository for message entities
pub struct MessageRepository {
    pool: Pool<Sqlite>,
}

impl MessageRepository {
    /// Create a new message repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new message
    pub async fn create(&self, message: &Message) -> Result<()> {
        debug!(
            "Creating message: {} -> {:?}",
            message.id, message.recipient_id
        );

        let metadata_json = serde_json::to_string(&message.metadata)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize metadata: {}", e)))?;

        let message_type_str = self.serialize_message_type(&message.message_type);
        let recipient_id_str = message.recipient_id.map(|id| id.to_string());
        let delivered_at_str = message.delivered_at.map(|dt| dt.to_rfc3339());
        let message_id_str = message.id.to_string();
        let sender_id_str = message.sender_id.to_string();
        let created_at_str = message.created_at.to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO messages (id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            message_id_str,
            sender_id_str,
            recipient_id_str,
            message_type_str,
            message.content,
            metadata_json,
            created_at_str,
            delivered_at_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        info!("Successfully created message: {}", message.id);
        Ok(())
    }

    /// Find a message by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>> {
        debug!("Finding message by ID: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages WHERE id = ?1",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        match row {
            Some(row) => {
                let message = self.parse_message_from_row(
                    &row.id.unwrap(),
                    &row.sender_id,
                    row.recipient_id.as_deref(),
                    &row.message_type,
                    &row.content,
                    &row.metadata,
                    &row.created_at,
                    row.delivered_at.as_deref(),
                )?;
                Ok(Some(message))
            }
            None => Ok(None),
        }
    }

    /// Update a message (primarily for marking as delivered)
    pub async fn update(&self, message: &Message) -> Result<()> {
        debug!("Updating message: {}", message.id);

        let metadata_json = serde_json::to_string(&message.metadata)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to serialize metadata: {}", e)))?;

        let message_type_str = self.serialize_message_type(&message.message_type);
        let _recipient_id_str = message.recipient_id.map(|id| id.to_string());
        let delivered_at_str = message.delivered_at.map(|dt| dt.to_rfc3339());
        let message_id_str = message.id.to_string();

        let rows_affected = sqlx::query!(
            r#"
            UPDATE messages 
            SET message_type = ?2, content = ?3, metadata = ?4, delivered_at = ?5
            WHERE id = ?1
            "#,
            message_id_str,
            message_type_str,
            message.content,
            metadata_json,
            delivered_at_str
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(Error::NotFound {
                entity: "Message".to_string(),
                id: message.id.to_string(),
            });
        }

        info!("Successfully updated message: {}", message.id);
        Ok(())
    }

    /// Delete a message
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        debug!("Deleting message with ID: {}", id);

        let id_str = id.to_string();
        let rows_affected = sqlx::query!("DELETE FROM messages WHERE id = ?1", id_str)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?
            .rows_affected();

        if rows_affected == 0 {
            return Err(Error::NotFound {
                entity: "Message".to_string(),
                id: id.to_string(),
            });
        }

        info!("Successfully deleted message with ID: {}", id);
        Ok(())
    }

    /// List messages for a recipient
    pub async fn list_for_recipient(&self, recipient_id: Uuid) -> Result<Vec<Message>> {
        debug!("Listing messages for recipient: {}", recipient_id);

        let recipient_id_str = recipient_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages WHERE recipient_id = ?1 ORDER BY created_at DESC",
            recipient_id_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut messages = Vec::new();
        for row in rows {
            let message = self.parse_message_from_row(
                &row.id.unwrap(),
                &row.sender_id,
                row.recipient_id.as_deref(),
                &row.message_type,
                &row.content,
                &row.metadata,
                &row.created_at,
                row.delivered_at.as_deref(),
            )?;
            messages.push(message);
        }

        debug!(
            "Found {} messages for recipient {}",
            messages.len(),
            recipient_id
        );
        Ok(messages)
    }

    /// List messages from a sender
    pub async fn list_from_sender(&self, sender_id: Uuid) -> Result<Vec<Message>> {
        debug!("Listing messages from sender: {}", sender_id);

        let sender_id_str = sender_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages WHERE sender_id = ?1 ORDER BY created_at DESC",
            sender_id_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut messages = Vec::new();
        for row in rows {
            let message = self.parse_message_from_row(
                &row.id.unwrap(),
                &row.sender_id,
                row.recipient_id.as_deref(),
                &row.message_type,
                &row.content,
                &row.metadata,
                &row.created_at,
                row.delivered_at.as_deref(),
            )?;
            messages.push(message);
        }

        debug!(
            "Found {} messages from sender {}",
            messages.len(),
            sender_id
        );
        Ok(messages)
    }

    /// List broadcast messages
    pub async fn list_broadcast_messages(&self) -> Result<Vec<Message>> {
        debug!("Listing broadcast messages");

        let rows = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages WHERE recipient_id IS NULL ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut messages = Vec::new();
        for row in rows {
            let message = self.parse_message_from_row(
                &row.id.unwrap(),
                &row.sender_id,
                row.recipient_id.as_deref(),
                &row.message_type,
                &row.content,
                &row.metadata,
                &row.created_at,
                row.delivered_at.as_deref(),
            )?;
            messages.push(message);
        }

        debug!("Found {} broadcast messages", messages.len());
        Ok(messages)
    }

    /// List recent messages (last N messages)
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<Message>> {
        debug!("Listing {} recent messages", limit);

        let rows = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages ORDER BY created_at DESC LIMIT ?1",
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut messages = Vec::new();
        for row in rows {
            let message = self.parse_message_from_row(
                &row.id.unwrap(),
                &row.sender_id,
                row.recipient_id.as_deref(),
                &row.message_type,
                &row.content,
                &row.metadata,
                &row.created_at,
                row.delivered_at.as_deref(),
            )?;
            messages.push(message);
        }

        debug!("Found {} recent messages", messages.len());
        Ok(messages)
    }

    /// Find messages by type
    pub async fn find_by_type(&self, message_type: &MessageType) -> Result<Vec<Message>> {
        debug!("Finding messages by type: {:?}", message_type);

        let type_str = self.serialize_message_type(message_type);
        let rows = sqlx::query!(
            "SELECT id, sender_id, recipient_id, message_type, content, metadata, created_at, delivered_at FROM messages WHERE message_type = ?1 ORDER BY created_at DESC",
            type_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        let mut messages = Vec::new();
        for row in rows {
            let message = self.parse_message_from_row(
                &row.id.unwrap(),
                &row.sender_id,
                row.recipient_id.as_deref(),
                &row.message_type,
                &row.content,
                &row.metadata,
                &row.created_at,
                row.delivered_at.as_deref(),
            )?;
            messages.push(message);
        }

        debug!(
            "Found {} messages with type {:?}",
            messages.len(),
            message_type
        );
        Ok(messages)
    }

    /// Count messages
    pub async fn count(&self) -> Result<i64> {
        debug!("Counting messages");

        let row = sqlx::query!("SELECT COUNT(*) as count FROM messages")
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        let count = row.count as i64;
        debug!("Total messages count: {}", count);
        Ok(count)
    }

    /// Count undelivered messages
    pub async fn count_undelivered(&self) -> Result<i64> {
        debug!("Counting undelivered messages");

        let row = sqlx::query!("SELECT COUNT(*) as count FROM messages WHERE delivered_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        let count = row.count as i64;
        debug!("Undelivered messages count: {}", count);
        Ok(count)
    }

    /// Check if a message exists
    pub async fn exists(&self, id: Uuid) -> Result<bool> {
        debug!("Checking if message exists: {}", id);

        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT COUNT(*) as count FROM messages WHERE id = ?1",
            id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        let exists = row.count > 0;
        debug!("Message {} exists: {}", id, exists);
        Ok(exists)
    }

    /// Parse message from database row
    #[allow(clippy::too_many_arguments)]
    fn parse_message_from_row(
        &self,
        id: &str,
        sender_id: &str,
        recipient_id: Option<&str>,
        message_type: &str,
        content: &str,
        metadata: &str,
        created_at: &str,
        delivered_at: Option<&str>,
    ) -> Result<Message> {
        let id = Uuid::parse_str(id)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse message ID: {}", e)))?;

        let sender_id = Uuid::parse_str(sender_id)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse sender ID: {}", e)))?;

        let recipient_id = if let Some(recipient_id_str) = recipient_id {
            Some(Uuid::parse_str(recipient_id_str).map_err(|e| {
                Error::Internal(anyhow::anyhow!("Failed to parse recipient ID: {}", e))
            })?)
        } else {
            None
        };

        let message_type = self.deserialize_message_type(message_type)?;

        let metadata: MessageMetadata = serde_json::from_str(metadata).map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to deserialize metadata: {}", e))
        })?;

        let created_at = DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to parse created_at: {}", e)))?
            .with_timezone(&Utc);

        let delivered_at = if let Some(delivered_at_str) = delivered_at {
            Some(
                DateTime::parse_from_rfc3339(delivered_at_str)
                    .map_err(|e| {
                        Error::Internal(anyhow::anyhow!("Failed to parse delivered_at: {}", e))
                    })?
                    .with_timezone(&Utc),
            )
        } else {
            None
        };

        Ok(Message {
            id,
            sender_id,
            recipient_id,
            message_type,
            content: content.to_string(),
            metadata,
            created_at,
            delivered_at,
        })
    }

    /// Serialize message type to string
    fn serialize_message_type(&self, message_type: &MessageType) -> String {
        match message_type {
            MessageType::Direct => "Direct".to_string(),
            MessageType::Broadcast => "Broadcast".to_string(),
            MessageType::StatusUpdate => "StatusUpdate".to_string(),
            MessageType::IssueNotification => "IssueNotification".to_string(),
            MessageType::KnowledgeShare => "KnowledgeShare".to_string(),
        }
    }

    /// Deserialize message type from string
    fn deserialize_message_type(&self, type_str: &str) -> Result<MessageType> {
        match type_str {
            "Direct" => Ok(MessageType::Direct),
            "Broadcast" => Ok(MessageType::Broadcast),
            "StatusUpdate" => Ok(MessageType::StatusUpdate),
            "IssueNotification" => Ok(MessageType::IssueNotification),
            "KnowledgeShare" => Ok(MessageType::KnowledgeShare),
            _ => Err(Error::Internal(anyhow::anyhow!(
                "Unknown message type: {}",
                type_str
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    include!("message_tests.rs");
}
