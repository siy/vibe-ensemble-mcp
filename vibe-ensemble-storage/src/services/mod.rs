//! Service layer for business logic

pub mod agent;
pub mod issue;

pub use agent::{
    AgentPool, AgentPoolConfig, AgentPoolPerformance, AgentPoolStatistics, AgentPoolStatus,
    AgentService, AgentSession, AgentStatistics, CapabilityStats, HealthCheckResult,
    LoadBalancerRecommendation, SystemHealth, TaskAssignment,
};

pub use issue::{AssignmentRecommendation, IssueService, IssueStatistics, WorkflowTransition};
