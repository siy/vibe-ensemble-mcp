//! Common test utilities and fixtures for the vibe-ensemble-mcp test suite.
//!
//! This module provides shared testing infrastructure including:
//! - Test database setup and teardown
//! - Mock data generation
//! - Test agent factories
//! - Common assertions and helpers

use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use fake::{Fake, Faker};
use sqlx::{Pool, Sqlite, SqlitePool};
use tempfile::NamedTempFile;

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus, ConnectionMetadata},
    issue::{Issue, IssueStatus, IssuePriority},
    message::{Message, MessageType},
    knowledge::{Knowledge, KnowledgeType, AccessLevel},
    config::Configuration,
};

pub mod agents;
pub mod database;
pub mod fixtures;
pub mod assertions;

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
        sqlx::migrate!("../vibe-ensemble-storage/migrations").run(&db_pool).await?;
        
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
                host: "localhost".to_string(),
                port: 8080,
                protocol: "http".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: Uuid::new_v4().to_string(),
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
        if let Some(recipient) = recipient_id {
            Message::direct(sender_id, recipient, "Test message content")
                .expect("Failed to create direct message")
        } else {
            Message::broadcast(sender_id, "Test broadcast message")
                .expect("Failed to create broadcast message")
        }
    }

    /// Creates test knowledge entry
    pub fn create_test_knowledge(&self, created_by: Uuid) -> Knowledge {
        Knowledge::builder()
            .title(format!("Test Knowledge {}", Uuid::new_v4()))
            .content("This is test knowledge content for automated testing")
            .knowledge_type(KnowledgeType::BestPractice)
            .access_level(AccessLevel::TeamVisible)
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
            .name(format!("agent-{}", fake::faker::name::en::Name().fake::<String>().to_lowercase().replace(" ", "-")))
            .connection_metadata(ConnectionMetadata {
                host: fake::faker::internet::en::DomainSuffix().fake(),
                port: (8000..9000).fake(),
                protocol: "https".to_string(),
                last_heartbeat: fake::faker::chrono::en::DateTimeBetween(
                    DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                    Utc::now()
                ).fake(),
                connection_id: Uuid::new_v4().to_string(),
            })
            .capabilities(vec![
                "coding".to_string(),
                "analysis".to_string(), 
                "review".to_string()
            ])
            .build()
            .expect("Failed to generate realistic agent")
    }
}

impl TestDataGenerator for Issue {
    fn generate_realistic() -> Self {
        Issue::builder()
            .title(fake::faker::lorem::en::Sentence(5..10).fake())
            .description(fake::faker::lorem::en::Paragraphs(1..3).fake::<Vec<String>>().join("\n\n"))
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
            let ctx = TestContext::new().await.expect("Failed to create test context");
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