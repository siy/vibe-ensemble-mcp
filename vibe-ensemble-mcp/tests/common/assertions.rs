//! Custom assertion helpers for vibe-ensemble-mcp testing
//!
//! Provides domain-specific assertions for testing complex scenarios.

use chrono::{DateTime, Utc};
use std::time::Duration;
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::{Agent, AgentStatus},
    issue::{Issue, IssueStatus},
    knowledge::Knowledge,
    message::Message,
};

/// Assertion helpers for agent-related tests
pub struct AgentAssertions;

impl AgentAssertions {
    /// Asserts that an agent is in a healthy state
    pub fn assert_agent_healthy(agent: &Agent) {
        assert_eq!(agent.status(), AgentStatus::Active);
        assert!(agent.capabilities().len() > 0);
        assert!(!agent.name().is_empty());
        assert!(agent.connection_metadata().last_heartbeat <= Utc::now());
    }

    /// Asserts that agents can communicate with each other
    pub fn assert_agents_can_communicate(agent1: &Agent, agent2: &Agent) {
        assert_eq!(agent1.status(), AgentStatus::Active);
        assert_eq!(agent2.status(), AgentStatus::Active);
        assert_ne!(agent1.id(), agent2.id());

        // Both agents should have active connections
        assert!(!agent1.connection_metadata().connection_id.is_empty());
        assert!(!agent2.connection_metadata().connection_id.is_empty());
    }

    /// Asserts that an agent has required capabilities
    pub fn assert_agent_has_capabilities(agent: &Agent, required_capabilities: &[&str]) {
        for capability in required_capabilities {
            assert!(
                agent.capabilities().iter().any(|c| c == capability),
                "Agent {} missing required capability: {}",
                agent.name(),
                capability
            );
        }
    }

    /// Asserts that agent uptime is within expected range
    pub fn assert_agent_uptime_reasonable(agent: &Agent, max_uptime: Duration) {
        let uptime = agent.uptime();
        assert!(
            uptime <= max_uptime,
            "Agent uptime {} exceeds maximum expected {}",
            uptime.as_secs(),
            max_uptime.as_secs()
        );
    }
}

/// Assertion helpers for issue-related tests
pub struct IssueAssertions;

impl IssueAssertions {
    /// Asserts that an issue is properly formed
    pub fn assert_issue_well_formed(issue: &Issue) {
        assert!(!issue.title().is_empty());
        assert!(!issue.description().is_empty());
        assert!(issue.created_at() <= Utc::now());
    }

    /// Asserts that an issue can transition between states
    pub fn assert_valid_status_transition(from: IssueStatus, to: IssueStatus) {
        match (from, to) {
            (IssueStatus::Open, IssueStatus::InProgress) => {}
            (IssueStatus::Open, IssueStatus::Blocked) => {}
            (IssueStatus::InProgress, IssueStatus::Blocked) => {}
            (IssueStatus::InProgress, IssueStatus::InReview) => {}
            (IssueStatus::InReview, IssueStatus::Done) => {}
            (IssueStatus::InReview, IssueStatus::InProgress) => {}
            (IssueStatus::Blocked, IssueStatus::Open) => {}
            (IssueStatus::Blocked, IssueStatus::InProgress) => {}
            (IssueStatus::Done, _) => panic!("Cannot transition from Done status"),
            _ => panic!("Invalid status transition from {:?} to {:?}", from, to),
        }
    }

    /// Asserts that an issue has been assigned and is being worked on
    pub fn assert_issue_actively_assigned(issue: &Issue) {
        assert!(issue.assigned_to().is_some());
        assert_ne!(issue.status(), IssueStatus::Open);

        if issue.status() == IssueStatus::InProgress {
            assert!(issue.started_at().is_some());
        }
    }

    /// Asserts that an issue resolution time is reasonable
    pub fn assert_reasonable_resolution_time(issue: &Issue, max_resolution_time: Duration) {
        if issue.status() == IssueStatus::Done {
            let resolution_time = issue
                .time_to_resolution()
                .expect("Completed issue should have resolution time");

            assert!(
                resolution_time <= max_resolution_time,
                "Issue resolution time {} exceeds maximum {}",
                resolution_time.as_secs(),
                max_resolution_time.as_secs()
            );
        }
    }
}

/// Assertion helpers for message-related tests
pub struct MessageAssertions;

impl MessageAssertions {
    /// Asserts that a message is properly formatted
    pub fn assert_message_well_formed(message: &Message) {
        assert!(!message.content().is_empty());
        assert!(message.created_at() <= Utc::now());
        assert!(!message.sender_id().is_nil());
    }

    /// Asserts that a message was delivered successfully
    pub fn assert_message_delivered(message: &Message) {
        assert!(message.delivered_at().is_some());
        assert!(message.delivered_at().unwrap() >= message.created_at());
    }

    /// Asserts that messages are ordered chronologically
    pub fn assert_messages_chronological(messages: &[Message]) {
        for window in messages.windows(2) {
            assert!(
                window[0].created_at() <= window[1].created_at(),
                "Messages are not in chronological order"
            );
        }
    }

    /// Asserts that broadcast messages have no specific recipient
    pub fn assert_broadcast_message(message: &Message) {
        assert!(message.recipient_id().is_none());
        assert!(message.content().len() > 0);
    }

    /// Asserts that direct messages have a specific recipient
    pub fn assert_direct_message(message: &Message, expected_recipient: Uuid) {
        assert_eq!(message.recipient_id(), Some(expected_recipient));
        assert!(!message.sender_id().is_nil());
        assert_ne!(message.sender_id(), expected_recipient);
    }
}

/// Assertion helpers for knowledge-related tests
pub struct KnowledgeAssertions;

impl KnowledgeAssertions {
    /// Asserts that knowledge entry is complete and valid
    pub fn assert_knowledge_complete(knowledge: &Knowledge) {
        assert!(!knowledge.title().is_empty());
        assert!(!knowledge.content().is_empty());
        assert!(!knowledge.created_by().is_nil());
        assert!(knowledge.created_at() <= Utc::now());
    }

    /// Asserts that knowledge is searchable
    pub fn assert_knowledge_searchable(knowledge: &Knowledge) {
        Self::assert_knowledge_complete(knowledge);
        assert!(knowledge.tags().len() > 0 || knowledge.title().len() > 10);
    }

    /// Asserts that knowledge access level is appropriate
    pub fn assert_appropriate_access_level(knowledge: &Knowledge, expected_visibility: bool) {
        use vibe_ensemble_core::knowledge::AccessLevel;

        match knowledge.access_level() {
            AccessLevel::Private => assert!(!expected_visibility),
            AccessLevel::TeamVisible | AccessLevel::PublicVisible => assert!(expected_visibility),
        }
    }
}

/// Performance assertion helpers
pub struct PerformanceAssertions;

impl PerformanceAssertions {
    /// Asserts that an operation completes within a time limit
    pub fn assert_completes_within<F, R>(max_duration: Duration, operation: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = operation();
        let elapsed = start.elapsed();

        assert!(
            elapsed <= max_duration,
            "Operation took {} ms, expected <= {} ms",
            elapsed.as_millis(),
            max_duration.as_millis()
        );

        result
    }

    /// Asserts that async operation completes within time limit
    pub async fn assert_async_completes_within<F, Fut, R>(max_duration: Duration, operation: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = std::time::Instant::now();
        let result = operation().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed <= max_duration,
            "Async operation took {} ms, expected <= {} ms",
            elapsed.as_millis(),
            max_duration.as_millis()
        );

        result
    }

    /// Asserts that memory usage is within acceptable bounds
    #[cfg(feature = "memory-profiling")]
    pub fn assert_memory_within_bounds<F, R>(max_memory_mb: usize, operation: F) -> R
    where
        F: FnOnce() -> R,
    {
        let initial_memory = Self::get_memory_usage();
        let result = operation();
        let final_memory = Self::get_memory_usage();

        let memory_used = final_memory.saturating_sub(initial_memory);
        assert!(
            memory_used <= max_memory_mb * 1024 * 1024,
            "Operation used {} MB, expected <= {} MB",
            memory_used / 1024 / 1024,
            max_memory_mb
        );

        result
    }

    #[cfg(feature = "memory-profiling")]
    fn get_memory_usage() -> usize {
        // This would use a memory profiling crate in production
        // For now, return 0 as placeholder
        0
    }
}

/// Macro for creating custom domain assertions
#[macro_export]
macro_rules! assert_domain {
    ($condition:expr, $($arg:tt)*) => {
        if !$condition {
            panic!("Domain assertion failed: {}", format!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::fixtures::{TestDataFactory, TestScenarios};

    #[test]
    fn test_agent_assertions() {
        let scenario = TestScenarios::development_team();
        let coordinator = &scenario.coordinator;

        AgentAssertions::assert_agent_healthy(coordinator);
        AgentAssertions::assert_agent_has_capabilities(
            coordinator,
            &["project_management", "task_coordination"],
        );
        AgentAssertions::assert_agent_uptime_reasonable(coordinator, Duration::from_secs(86400));
    }

    #[test]
    fn test_issue_assertions() {
        let issue = TestDataFactory::create_random_issue();
        IssueAssertions::assert_issue_well_formed(&issue);
    }

    #[test]
    fn test_message_assertions() {
        let agents = TestScenarios::development_team();
        let messages = TestScenarios::message_exchanges(&agents.all_agents());

        for message in &messages {
            MessageAssertions::assert_message_well_formed(message);
        }

        MessageAssertions::assert_messages_chronological(&messages);
    }

    #[test]
    fn test_performance_assertions() {
        // Test synchronous operation timing
        let result =
            PerformanceAssertions::assert_completes_within(Duration::from_millis(100), || {
                std::thread::sleep(Duration::from_millis(10));
                42
            });
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_async_performance_assertions() {
        let result = PerformanceAssertions::assert_async_completes_within(
            Duration::from_millis(100),
            || async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                42
            },
        )
        .await;
        assert_eq!(result, 42);
    }

    #[test]
    #[should_panic(expected = "Domain assertion failed")]
    fn test_domain_assertion_macro() {
        assert_domain!(false, "This should fail with message");
    }
}
