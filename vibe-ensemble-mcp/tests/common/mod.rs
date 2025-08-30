//! Common test utilities and fixtures for the vibe-ensemble-mcp test suite.
//!
//! This module provides shared testing infrastructure including:
//! - Test database setup and teardown
//! - Mock data generation
//! - Test agent factories
//! - Common assertions and helpers

use chrono::{DateTime, Utc};
use fake::{Fake, Faker};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::Mutex;
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus, ConnectionMetadata},
    config::Configuration,
    issue::{Issue, IssuePriority, IssueStatus},
    knowledge::{AccessLevel, Knowledge, KnowledgeType},
    message::{Message, MessageType},
};

pub mod agents;
pub mod assertions;
pub mod database;
pub mod fixtures;

// Integration test framework modules
pub mod file_system_verifier;
pub mod framework;
pub mod mock_agents;
pub mod worktree_manager;

/// Test configuration and shared resources
pub struct TestContext {
    pub db_pool: Arc<SqlitePool>,
    pub temp_db: NamedTempFile,
}

impl TestContext {
    /// Creates a new test context with an in-memory SQLite database
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_db = NamedTempFile::new()?;
        let db_url = format!("sqlite:{}", temp_db.path().display());

        let db_pool = SqlitePool::connect(&db_url).await?;

        // Run migrations
        sqlx::migrate!("../vibe-ensemble-storage/migrations")
            .run(&db_pool)
            .await?;

        Ok(TestContext {
            db_pool: Arc::new(db_pool),
            temp_db,
        })
    }

    /// Creates a test agent with randomized data
    pub fn create_test_agent(&self) -> Agent {
        Agent::builder()
            .name(format!("test-agent-{}", Uuid::new_v4()))
            .connection_metadata(ConnectionMetadata {
                endpoint: "http://localhost:8080".to_string(),
                protocol_version: "1.0".to_string(),
                session_id: Some(Uuid::new_v4().to_string()),
            })
            .capabilities(vec!["test".to_string(), "mock".to_string()])
            .build()
            .expect("Failed to create test agent")
    }

    /// Creates a test issue with randomized data
    pub fn create_test_issue(&self) -> Issue {
        Issue::builder()
            .title(format!("Test Issue {}", Uuid::new_v4()))
            .description("This is a test issue for automated testing")
            .priority(IssuePriority::Medium)
            .build()
            .expect("Failed to create test issue")
    }

    /// Creates a test message
    pub fn create_test_message(&self, sender_id: Uuid, recipient_id: Option<Uuid>) -> Message {
        use vibe_ensemble_core::message::MessagePriority;
        if let Some(recipient) = recipient_id {
            Message::new_direct(sender_id, recipient, "Test message content".to_string(), MessagePriority::Normal)
                .expect("Failed to create direct message")
        } else {
            Message::new_broadcast(sender_id, "Test broadcast message".to_string(), MessagePriority::Normal)
                .expect("Failed to create broadcast message")
        }
    }

    /// Creates test knowledge entry
    pub fn create_test_knowledge(&self, created_by: Uuid) -> Knowledge {
        Knowledge::builder()
            .title(format!("Test Knowledge {}", Uuid::new_v4()))
            .content("This is test knowledge content for automated testing")
            .knowledge_type(KnowledgeType::Practice)
            .access_level(AccessLevel::Team)
            .created_by(created_by)
            .build()
            .expect("Failed to create test knowledge")
    }
}

/// Helper trait for generating test data with fake values
pub trait TestDataGenerator {
    fn generate_realistic() -> Self;
}

impl TestDataGenerator for Agent {
    fn generate_realistic() -> Self {
        Agent::builder()
            .name(format!(
                "agent-{}",
                fake::faker::name::en::Name()
                    .fake::<String>()
                    .to_lowercase()
                    .replace(" ", "-")
            ))
            .connection_metadata(ConnectionMetadata {
                endpoint: format!(
                    "https://{}",
                    fake::faker::internet::en::DomainSuffix().fake::<String>()
                ),
                protocol_version: "1.0".to_string(),
                session_id: Some(Uuid::new_v4().to_string()),
            })
            .capabilities(vec![
                "coding".to_string(),
                "analysis".to_string(),
                "review".to_string(),
            ])
            .build()
            .expect("Failed to generate realistic agent")
    }
}

impl TestDataGenerator for Issue {
    fn generate_realistic() -> Self {
        Issue::builder()
            .title(fake::faker::lorem::en::Sentence(5..10).fake::<String>())
            .description(
                fake::faker::lorem::en::Paragraphs(1..3)
                    .fake::<Vec<String>>()
                    .join("\n\n"),
            )
            .priority(IssuePriority::Medium)
            .build()
            .expect("Failed to generate realistic issue")
    }
}

/// Async test helper macros
#[macro_export]
macro_rules! async_test {
    ($test_name:ident, $test_body:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let ctx = TestContext::new()
                .await
                .expect("Failed to create test context");
            $test_body(ctx).await
        }
    };
}

/// Property-based test helper
#[macro_export]
macro_rules! prop_test {
    ($test_name:ident, $strategy:expr, $test_body:expr) => {
        proptest! {
            #[test]
            fn $test_name(input in $strategy) {
                $test_body(input)
            }
        }
    };
}
