//! Service layer for business logic

pub mod agent;
pub mod coordination;
pub mod issue;
pub mod knowledge;
pub mod knowledge_intelligence;
pub mod message;

pub use agent::{
    AgentPool, AgentPoolConfig, AgentPoolPerformance, AgentPoolStatistics, AgentPoolStatus,
    AgentService, AgentSession, AgentStatistics, CapabilityStats, HealthCheckResult,
    LoadBalancerRecommendation, SystemHealth, TaskAssignment,
};

pub use issue::{AssignmentRecommendation, IssueService, IssueStatistics, WorkflowTransition};

pub use knowledge::{KnowledgeService, KnowledgeStatistics};

pub use knowledge_intelligence::KnowledgeIntelligenceService;

pub use message::{
    DeliveryStatus, DeliveryStatusType, MessageEvent, MessageEventType, MessageService,
    MessageStatistics,
};

pub use coordination::CoordinationService;
