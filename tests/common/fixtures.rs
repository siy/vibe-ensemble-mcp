//! Test fixtures for vibe-ensemble-mcp
//!
//! Provides pre-configured test data and scenarios for various testing needs.

use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use fake::{Fake, faker::*};

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus, ConnectionMetadata},
    issue::{Issue, IssueStatus, IssuePriority},
    message::{Message, MessageType},
    knowledge::{Knowledge, KnowledgeType, AccessLevel},
    config::{Configuration, CoordinationSettings, RetryPolicy},
};

/// Predefined test scenarios for consistent testing
pub struct TestScenarios;

impl TestScenarios {
    /// Creates a typical multi-agent development team scenario
    pub fn development_team() -> DevTeamScenario {
        let coordinator = Agent::builder()
            .name("coordinator-agent")
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8080,
                protocol: "http".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: "coord-001".to_string(),
            })
            .capabilities(vec![
                "project_management".to_string(),
                "task_coordination".to_string(),
                "strategic_planning".to_string(),
            ])
            .build()
            .expect("Failed to create coordinator agent");

        let backend_dev = Agent::builder()
            .name("backend-developer")
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8081,
                protocol: "http".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: "backend-001".to_string(),
            })
            .capabilities(vec![
                "rust_development".to_string(),
                "database_design".to_string(),
                "api_development".to_string(),
            ])
            .build()
            .expect("Failed to create backend developer");

        let frontend_dev = Agent::builder()
            .name("frontend-developer")
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8082,
                protocol: "http".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: "frontend-001".to_string(),
            })
            .capabilities(vec![
                "web_development".to_string(),
                "ui_design".to_string(),
                "testing".to_string(),
            ])
            .build()
            .expect("Failed to create frontend developer");

        let qa_agent = Agent::builder()
            .name("qa-specialist")
            .connection_metadata(ConnectionMetadata {
                host: "localhost".to_string(),
                port: 8083,
                protocol: "http".to_string(),
                last_heartbeat: Utc::now(),
                connection_id: "qa-001".to_string(),
            })
            .capabilities(vec![
                "quality_assurance".to_string(),
                "automated_testing".to_string(),
                "performance_testing".to_string(),
            ])
            .build()
            .expect("Failed to create QA agent");

        DevTeamScenario {
            coordinator,
            backend_dev,
            frontend_dev,
            qa_agent,
        }
    }

    /// Creates a comprehensive issue backlog for testing
    pub fn issue_backlog() -> Vec<Issue> {
        vec![
            Issue::builder()
                .title("Implement user authentication system")
                .description("Design and implement secure user authentication with JWT tokens")
                .priority(IssuePriority::High)
                .build()
                .expect("Failed to create auth issue"),
            
            Issue::builder()
                .title("Add real-time messaging support")
                .description("Implement WebSocket-based real-time messaging between agents")
                .priority(IssuePriority::Medium)
                .build()
                .expect("Failed to create messaging issue"),
            
            Issue::builder()
                .title("Optimize database queries")
                .description("Review and optimize slow database queries for better performance")
                .priority(IssuePriority::Low)
                .build()
                .expect("Failed to create optimization issue"),
            
            Issue::builder()
                .title("Fix memory leak in message handling")
                .description("Investigate and fix reported memory leak in message processing")
                .priority(IssuePriority::Critical)
                .build()
                .expect("Failed to create critical issue"),
        ]
    }

    /// Creates a knowledge repository with various types of entries
    pub fn knowledge_repository(author_id: Uuid) -> Vec<Knowledge> {
        vec![
            Knowledge::builder()
                .title("Rust Best Practices Guide")
                .content("A comprehensive guide to writing idiomatic Rust code...")
                .knowledge_type(KnowledgeType::BestPractice)
                .access_level(AccessLevel::PublicVisible)
                .created_by(author_id)
                .build()
                .expect("Failed to create best practices knowledge"),
            
            Knowledge::builder()
                .title("Database Schema Design Patterns")
                .content("Common patterns for designing efficient database schemas...")
                .knowledge_type(KnowledgeType::TechnicalDocumentation)
                .access_level(AccessLevel::TeamVisible)
                .created_by(author_id)
                .build()
                .expect("Failed to create technical documentation"),
            
            Knowledge::builder()
                .title("Debugging Memory Issues")
                .content("Step-by-step guide for debugging memory leaks and performance issues...")
                .knowledge_type(KnowledgeType::TroubleshootingGuide)
                .access_level(AccessLevel::TeamVisible)
                .created_by(author_id)
                .build()
                .expect("Failed to create troubleshooting guide"),
        ]
    }

    /// Creates realistic message exchanges between agents
    pub fn message_exchanges(agents: &[Agent]) -> Vec<Message> {
        let mut messages = Vec::new();
        let agent_ids: Vec<Uuid> = agents.iter().map(|a| a.id()).collect();
        
        // Coordinator broadcasts project update
        messages.push(
            Message::broadcast(
                agent_ids[0],
                "Sprint planning meeting at 2 PM today. Please review the backlog items."
            ).expect("Failed to create broadcast message")
        );

        // Backend dev requests clarification
        messages.push(
            Message::direct(
                agent_ids[1],
                agent_ids[0],
                "Can you clarify the authentication requirements for the user system?"
            ).expect("Failed to create direct message")
        );

        // Frontend dev shares progress update
        messages.push(
            Message::broadcast(
                agent_ids[2],
                "UI mockups are ready for review. Updated designs in shared repository."
            ).expect("Failed to create progress update")
        );

        messages
    }
}

/// Development team scenario with specialized agents
pub struct DevTeamScenario {
    pub coordinator: Agent,
    pub backend_dev: Agent,
    pub frontend_dev: Agent,
    pub qa_agent: Agent,
}

impl DevTeamScenario {
    /// Returns all agents in the team
    pub fn all_agents(&self) -> Vec<&Agent> {
        vec![&self.coordinator, &self.backend_dev, &self.frontend_dev, &self.qa_agent]
    }

    /// Returns agent IDs
    pub fn agent_ids(&self) -> Vec<Uuid> {
        self.all_agents().into_iter().map(|a| a.id()).collect()
    }
}

/// Factory for creating random test data
pub struct TestDataFactory;

impl TestDataFactory {
    /// Creates a random agent with realistic properties
    pub fn create_random_agent() -> Agent {
        Agent::builder()
            .name(format!("agent-{}", name::en::FirstName().fake::<String>().to_lowercase()))
            .connection_metadata(ConnectionMetadata {
                host: internet::en::IPv4().fake::<std::net::Ipv4Addr>().to_string(),
                port: (8000..9000).fake(),
                protocol: "https".to_string(),
                last_heartbeat: chrono::en::DateTimeBetween(
                    DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
                    Utc::now()
                ).fake(),
                connection_id: Uuid::new_v4().to_string(),
            })
            .capabilities(vec![
                lorem::en::Word().fake(),
                lorem::en::Word().fake(),
                lorem::en::Word().fake(),
            ])
            .build()
            .expect("Failed to create random agent")
    }

    /// Creates a random issue with realistic content
    pub fn create_random_issue() -> Issue {
        let priorities = vec![IssuePriority::Low, IssuePriority::Medium, IssuePriority::High, IssuePriority::Critical];
        
        Issue::builder()
            .title(lorem::en::Sentence(4..8).fake())
            .description(lorem::en::Paragraphs(1..3).fake::<Vec<String>>().join("\n\n"))
            .priority(priorities[number::en::NumberWithinRange(0..priorities.len()).fake()])
            .build()
            .expect("Failed to create random issue")
    }

    /// Creates random knowledge entry
    pub fn create_random_knowledge(author_id: Uuid) -> Knowledge {
        let types = vec![
            KnowledgeType::BestPractice,
            KnowledgeType::TechnicalDocumentation,
            KnowledgeType::TroubleshootingGuide,
        ];
        
        let access_levels = vec![
            AccessLevel::Private,
            AccessLevel::TeamVisible,
            AccessLevel::PublicVisible,
        ];
        
        Knowledge::builder()
            .title(lorem::en::Sentence(3..6).fake())
            .content(lorem::en::Paragraphs(2..5).fake::<Vec<String>>().join("\n\n"))
            .knowledge_type(types[number::en::NumberWithinRange(0..types.len()).fake()])
            .access_level(access_levels[number::en::NumberWithinRange(0..access_levels.len()).fake()])
            .created_by(author_id)
            .build()
            .expect("Failed to create random knowledge")
    }

    /// Creates a batch of test data for load testing
    pub fn create_load_test_data(agent_count: usize, issue_count: usize, message_count: usize) -> LoadTestData {
        let mut agents = Vec::new();
        for _ in 0..agent_count {
            agents.push(Self::create_random_agent());
        }

        let mut issues = Vec::new();
        for _ in 0..issue_count {
            issues.push(Self::create_random_issue());
        }

        let mut messages = Vec::new();
        for _ in 0..message_count {
            let sender = agents[number::en::NumberWithinRange(0..agents.len()).fake()].id();
            let recipient = if boolean::en::Boolean(30).fake() {
                // 30% chance of broadcast message
                None
            } else {
                Some(agents[number::en::NumberWithinRange(0..agents.len()).fake()].id())
            };
            
            let message = if let Some(recipient_id) = recipient {
                Message::direct(sender, recipient_id, &lorem::en::Sentence(5..15).fake::<String>())
            } else {
                Message::broadcast(sender, &lorem::en::Sentence(5..15).fake::<String>())
            };
            
            messages.push(message.expect("Failed to create test message"));
        }

        LoadTestData {
            agents,
            issues,
            messages,
        }
    }
}

/// Data structure for load testing scenarios
pub struct LoadTestData {
    pub agents: Vec<Agent>,
    pub issues: Vec<Issue>,
    pub messages: Vec<Message>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_development_team_scenario() {
        let scenario = TestScenarios::development_team();
        assert_eq!(scenario.all_agents().len(), 4);
        assert_eq!(scenario.agent_ids().len(), 4);
        
        // Verify agent roles
        assert!(scenario.coordinator.capabilities().contains(&"project_management".to_string()));
        assert!(scenario.backend_dev.capabilities().contains(&"rust_development".to_string()));
        assert!(scenario.frontend_dev.capabilities().contains(&"web_development".to_string()));
        assert!(scenario.qa_agent.capabilities().contains(&"quality_assurance".to_string()));
    }

    #[test]
    fn test_issue_backlog() {
        let issues = TestScenarios::issue_backlog();
        assert_eq!(issues.len(), 4);
        
        // Verify we have different priority levels
        let priorities: std::collections::HashSet<_> = issues.iter().map(|i| i.priority()).collect();
        assert!(priorities.len() > 1);
    }

    #[test]
    fn test_knowledge_repository() {
        let author_id = Uuid::new_v4();
        let knowledge = TestScenarios::knowledge_repository(author_id);
        assert_eq!(knowledge.len(), 3);
        
        // Verify all entries have the same author
        assert!(knowledge.iter().all(|k| k.created_by() == author_id));
        
        // Verify different knowledge types
        let types: std::collections::HashSet<_> = knowledge.iter().map(|k| k.knowledge_type()).collect();
        assert!(types.len() > 1);
    }

    #[test]
    fn test_random_data_generation() {
        let agent = TestDataFactory::create_random_agent();
        let issue = TestDataFactory::create_random_issue();
        let knowledge = TestDataFactory::create_random_knowledge(Uuid::new_v4());
        
        // Basic validation
        assert!(!agent.name().is_empty());
        assert!(!issue.title().is_empty());
        assert!(!knowledge.title().is_empty());
    }

    #[test]
    fn test_load_test_data_creation() {
        let data = TestDataFactory::create_load_test_data(5, 10, 20);
        assert_eq!(data.agents.len(), 5);
        assert_eq!(data.issues.len(), 10);
        assert_eq!(data.messages.len(), 20);
    }
}