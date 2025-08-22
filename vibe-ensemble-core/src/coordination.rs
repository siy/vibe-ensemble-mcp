//! Cross-project dependency coordination models and types
//!
//! This module provides domain models for coordinating dependencies and work
//! between multiple Claude Code agents across different projects. It enables
//! sophisticated coordination scenarios including dependency declaration,
//! worker spawning requests, work coordination, and conflict resolution.
//!
//! # Core Concepts
//!
//! - **Dependency Declaration**: Workers can declare when their work depends on changes in other projects
//! - **Worker Coordination**: Active workers can negotiate work ordering and resource sharing
//! - **Coordinator Requests**: Workers can request the coordinator spawn new workers for other projects
//! - **Conflict Resolution**: Handle overlapping modifications and resource conflicts
//!
//! # Example Usage
//!
//! ```rust
//! use vibe_ensemble_core::coordination::*;
//! use uuid::Uuid;
//!
//! // Worker declares a dependency on another project
//! let dependency = CrossProjectDependency::builder()
//!     .declaring_agent_id(Uuid::new_v4())
//!     .source_project("frontend-app")
//!     .target_project("api-server")
//!     .dependency_type(DependencyType::ApiChange)
//!     .description("Need API endpoint for user preferences")
//!     .impact(DependencyImpact::Blocker)
//!     .urgency(DependencyUrgency::High)
//!     .build()
//!     .unwrap();
//!
//! // Worker requests coordinator spawn a new worker
//! let request = WorkerSpawnRequest::builder()
//!     .requesting_agent_id(Uuid::new_v4())
//!     .target_project("api-server")
//!     .required_capabilities(vec!["backend-development", "api-design"])
//!     .priority(SpawnPriority::High)
//!     .task_description("Implement user preferences API endpoints")
//!     .build()
//!     .unwrap();
//! ```

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Cross-project dependency tracking information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrossProjectDependency {
    pub id: Uuid,
    pub declaring_agent_id: Uuid,
    pub source_project: String,
    pub target_project: String,
    pub dependency_type: DependencyType,
    pub description: String,
    pub impact: DependencyImpact,
    pub urgency: DependencyUrgency,
    pub affected_files: Vec<String>,
    pub status: DependencyStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub coordination_plan: Option<CoordinationPlan>,
    pub metadata: HashMap<String, String>,
}

/// Types of cross-project dependencies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyType {
    /// Changes to interfaces/contracts between projects
    ApiChange,
    /// Common files/modules used by multiple projects
    SharedResource,
    /// Changes to build system or dependencies
    BuildDependency,
    /// Shared configuration or environment changes
    Configuration,
    /// Database or data format changes
    DataSchema,
    /// Custom dependency type
    Custom(String),
}

/// Impact level of a dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyImpact {
    /// Work cannot proceed without resolving this dependency
    Blocker,
    /// Work can proceed but with reduced functionality
    Major,
    /// Work can proceed with minor impact
    Minor,
    /// Informational dependency, no blocking impact
    Info,
}

/// Urgency level of a dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyUrgency {
    /// Needs immediate attention
    Critical,
    /// Should be resolved soon
    High,
    /// Normal priority
    Medium,
    /// Can be resolved later
    Low,
}

/// Status of a dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyStatus {
    /// Dependency has been declared
    Declared,
    /// Coordination plan has been created
    Planned,
    /// Work is in progress
    InProgress,
    /// Dependency has been resolved
    Resolved,
    /// Dependency was cancelled
    Cancelled,
    /// Dependency resolution failed
    Failed { reason: String },
}

/// Coordination plan for resolving a dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationPlan {
    pub id: Uuid,
    pub dependency_id: Uuid,
    pub plan_type: CoordinationPlanType,
    pub required_actions: Vec<RequiredAction>,
    pub estimated_duration: Option<chrono::Duration>,
    pub assigned_agents: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub timeline: Option<CoordinationTimeline>,
}

/// Type of coordination plan
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoordinationPlanType {
    /// Direct coordination between existing active workers
    DirectCoordination,
    /// Request coordinator to spawn new worker
    WorkerSpawn,
    /// Sequential work ordering
    Sequential,
    /// Parallel work with synchronization points
    Parallel,
    /// Conflict resolution required
    ConflictResolution,
}

/// Required action for resolving a dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequiredAction {
    pub id: Uuid,
    pub action_type: ActionType,
    pub description: String,
    pub assigned_agent_id: Option<Uuid>,
    pub target_project: String,
    pub affected_resources: Vec<String>,
    pub estimated_effort: Option<String>,
    pub dependencies: Vec<Uuid>, // Other actions this depends on
    pub status: ActionStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Type of required action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    /// Implement new functionality
    Implement,
    /// Modify existing code
    Modify,
    /// Review changes
    Review,
    /// Test changes
    Test,
    /// Deploy changes
    Deploy,
    /// Document changes
    Document,
    /// Coordinate with other agents
    Coordinate,
}

/// Status of a required action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionStatus {
    Pending,
    InProgress,
    Completed,
    Failed { reason: String },
    Cancelled,
}

/// Timeline for coordination activities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationTimeline {
    pub milestones: Vec<CoordinationMilestone>,
    pub estimated_completion: DateTime<Utc>,
    pub critical_path: Vec<Uuid>, // IDs of actions on critical path
}

/// Milestone in coordination timeline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationMilestone {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub target_date: DateTime<Utc>,
    pub dependencies: Vec<Uuid>,
    pub status: MilestoneStatus,
}

/// Status of a coordination milestone
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MilestoneStatus {
    NotStarted,
    InProgress,
    Completed,
    Delayed { reason: String },
    AtRisk,
}

/// Request for coordinator to spawn a new worker
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerSpawnRequest {
    pub id: Uuid,
    pub requesting_agent_id: Uuid,
    pub target_project: String,
    pub required_capabilities: Vec<String>,
    pub priority: SpawnPriority,
    pub task_description: String,
    pub estimated_duration: Option<chrono::Duration>,
    pub context_data: HashMap<String, String>,
    pub status: SpawnRequestStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assigned_worker_id: Option<Uuid>,
    pub spawn_result: Option<SpawnResult>,
}

/// Priority level for worker spawn requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpawnPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// Status of a worker spawn request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpawnRequestStatus {
    Pending,
    Evaluating,
    Approved,
    InProgress,
    Completed,
    Rejected { reason: String },
    Failed { reason: String },
}

/// Result of a worker spawn operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpawnResult {
    pub worker_id: Uuid,
    pub spawn_time: DateTime<Utc>,
    pub estimated_availability: DateTime<Utc>,
    pub capabilities_confirmed: Vec<String>,
    pub initial_context: HashMap<String, String>,
}

/// Work coordination agreement between agents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkCoordinationAgreement {
    pub id: Uuid,
    pub initiating_agent_id: Uuid,
    pub target_agent_id: Uuid,
    pub coordination_type: WorkCoordinationType,
    pub work_items: Vec<WorkItem>,
    pub dependencies: Vec<WorkDependency>,
    pub negotiated_timeline: CoordinationTimeline,
    pub status: CoordinationStatus,
    pub created_at: DateTime<Utc>,
    pub agreed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub terms: CoordinationTerms,
}

/// Type of work coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkCoordinationType {
    /// Sequential work ordering
    Sequential,
    /// Parallel work with resource sharing
    Parallel,
    /// One agent blocks for another
    Blocking,
    /// Collaborative work on shared resources
    Collaborative,
    /// Resource conflict resolution
    ConflictResolution,
}

/// Individual work item in coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkItem {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub assigned_agent_id: Uuid,
    pub estimated_effort: Option<String>,
    pub resources: Vec<String>,
    pub dependencies: Vec<Uuid>,
    pub status: WorkItemStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// Status of a work item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkItemStatus {
    Planned,
    Ready,
    InProgress,
    Completed,
    Blocked { reason: String },
    Failed { reason: String },
}

/// Dependency between work items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkDependency {
    pub id: Uuid,
    pub source_item_id: Uuid,
    pub target_item_id: Uuid,
    pub dependency_type: WorkDependencyType,
    pub description: String,
}

/// Type of work dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkDependencyType {
    /// Target must complete before source can start
    FinishToStart,
    /// Target must start before source can finish
    StartToFinish,
    /// Both items must start together
    StartToStart,
    /// Both items must finish together
    FinishToFinish,
}

/// Status of work coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoordinationStatus {
    Proposed,
    UnderNegotiation,
    Agreed,
    InProgress,
    Completed,
    Cancelled,
    Failed { reason: String },
}

/// Terms and conditions of coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationTerms {
    pub resource_allocation: HashMap<String, String>,
    pub communication_protocol: CommunicationProtocol,
    pub escalation_rules: Vec<EscalationRule>,
    pub success_criteria: Vec<String>,
    pub failure_conditions: Vec<String>,
}

/// Communication protocol for coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommunicationProtocol {
    pub update_frequency: String,
    pub notification_triggers: Vec<String>,
    pub escalation_timeout: Option<chrono::Duration>,
}

/// Rule for escalating coordination issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EscalationRule {
    pub condition: String,
    pub action: EscalationAction,
    pub timeout: Option<chrono::Duration>,
}

/// Action to take when escalating
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EscalationAction {
    NotifyCoordinator,
    RequestMediation,
    AbortCoordination,
    RenegotiateTerms,
}

/// Conflict resolution case
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictResolutionCase {
    pub id: Uuid,
    pub affected_agents: Vec<Uuid>,
    pub conflicted_resources: Vec<String>,
    pub conflict_type: ConflictType,
    pub resolution_strategy: Option<ResolutionStrategy>,
    pub resolver_agent_id: Option<Uuid>,
    pub status: ConflictStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_plan: Option<ConflictResolutionPlan>,
    pub evidence: Vec<ConflictEvidence>,
}

/// Type of conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictType {
    /// Multiple agents modifying same files
    FileModification,
    /// Conflicting resource locks
    ResourceLock,
    /// Incompatible architectural changes
    Architecture,
    /// Conflicting business logic
    BusinessLogic,
    /// Testing conflicts
    Testing,
    /// Deployment conflicts
    Deployment,
}

/// Strategy for resolving conflicts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionStrategy {
    /// Last writer wins
    LastWriterWins,
    /// First writer wins
    FirstWriterWins,
    /// Merge changes automatically
    AutoMerge,
    /// Manual merge required
    ManualMerge,
    /// Split resources
    ResourceSplit,
    /// Sequential execution
    Sequential,
    /// Escalate to coordinator
    Escalate,
}

/// Status of conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictStatus {
    Detected,
    Analyzing,
    ResolutionPlanned,
    InProgress,
    Resolved,
    Escalated,
    Failed { reason: String },
}

/// Plan for resolving a conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictResolutionPlan {
    pub id: Uuid,
    pub conflict_id: Uuid,
    pub strategy: ResolutionStrategy,
    pub steps: Vec<ResolutionStep>,
    pub required_actions_per_agent: HashMap<Uuid, Vec<RequiredAction>>,
    pub estimated_resolution_time: Option<chrono::Duration>,
    pub rollback_plan: Option<RollbackPlan>,
}

/// Step in conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolutionStep {
    pub id: Uuid,
    pub sequence: i32,
    pub description: String,
    pub assigned_agent_id: Option<Uuid>,
    pub estimated_duration: Option<chrono::Duration>,
    pub dependencies: Vec<Uuid>,
    pub status: ActionStatus,
}

/// Plan for rolling back changes if resolution fails
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RollbackPlan {
    pub steps: Vec<RollbackStep>,
    pub trigger_conditions: Vec<String>,
    pub estimated_rollback_time: Option<chrono::Duration>,
}

/// Step in rollback process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RollbackStep {
    pub id: Uuid,
    pub sequence: i32,
    pub description: String,
    pub agent_id: Uuid,
    pub resources: Vec<String>,
}

/// Evidence of a conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictEvidence {
    pub id: Uuid,
    pub evidence_type: EvidenceType,
    pub description: String,
    pub source_agent_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

/// Type of conflict evidence
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvidenceType {
    FileModification,
    ResourceLock,
    ErrorLog,
    TestFailure,
    AgentReport,
    SystemMetrics,
}

// Builder implementations
impl CrossProjectDependency {
    pub fn builder() -> CrossProjectDependencyBuilder {
        CrossProjectDependencyBuilder::new()
    }
}

impl WorkerSpawnRequest {
    pub fn builder() -> WorkerSpawnRequestBuilder {
        WorkerSpawnRequestBuilder::new()
    }
}

impl WorkCoordinationAgreement {
    pub fn builder() -> WorkCoordinationAgreementBuilder {
        WorkCoordinationAgreementBuilder::new()
    }
}

impl ConflictResolutionCase {
    pub fn builder() -> ConflictResolutionCaseBuilder {
        ConflictResolutionCaseBuilder::new()
    }
}

// Builder structures
#[derive(Debug, Default)]
pub struct CrossProjectDependencyBuilder {
    declaring_agent_id: Option<Uuid>,
    source_project: Option<String>,
    target_project: Option<String>,
    dependency_type: Option<DependencyType>,
    description: Option<String>,
    impact: Option<DependencyImpact>,
    urgency: Option<DependencyUrgency>,
    affected_files: Vec<String>,
    metadata: HashMap<String, String>,
}

impl CrossProjectDependencyBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declaring_agent_id(mut self, agent_id: Uuid) -> Self {
        self.declaring_agent_id = Some(agent_id);
        self
    }

    pub fn source_project<S: Into<String>>(mut self, project: S) -> Self {
        self.source_project = Some(project.into());
        self
    }

    pub fn target_project<S: Into<String>>(mut self, project: S) -> Self {
        self.target_project = Some(project.into());
        self
    }

    pub fn dependency_type(mut self, dep_type: DependencyType) -> Self {
        self.dependency_type = Some(dep_type);
        self
    }

    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn impact(mut self, impact: DependencyImpact) -> Self {
        self.impact = Some(impact);
        self
    }

    pub fn urgency(mut self, urgency: DependencyUrgency) -> Self {
        self.urgency = Some(urgency);
        self
    }

    pub fn affected_file<S: Into<String>>(mut self, file: S) -> Self {
        self.affected_files.push(file.into());
        self
    }

    pub fn affected_files<I, S>(mut self, files: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.affected_files
            .extend(files.into_iter().map(|f| f.into()));
        self
    }

    pub fn metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Result<CrossProjectDependency> {
        let now = Utc::now();
        Ok(CrossProjectDependency {
            id: Uuid::new_v4(),
            declaring_agent_id: self
                .declaring_agent_id
                .ok_or_else(|| Error::validation("declaring_agent_id is required"))?,
            source_project: self
                .source_project
                .ok_or_else(|| Error::validation("source_project is required"))?,
            target_project: self
                .target_project
                .ok_or_else(|| Error::validation("target_project is required"))?,
            dependency_type: self
                .dependency_type
                .ok_or_else(|| Error::validation("dependency_type is required"))?,
            description: self
                .description
                .ok_or_else(|| Error::validation("description is required"))?,
            impact: self.impact.unwrap_or(DependencyImpact::Major),
            urgency: self.urgency.unwrap_or(DependencyUrgency::Medium),
            affected_files: self.affected_files,
            status: DependencyStatus::Declared,
            created_at: now,
            updated_at: now,
            resolved_at: None,
            coordination_plan: None,
            metadata: self.metadata,
        })
    }
}

#[derive(Debug, Default)]
pub struct WorkerSpawnRequestBuilder {
    requesting_agent_id: Option<Uuid>,
    target_project: Option<String>,
    required_capabilities: Vec<String>,
    priority: Option<SpawnPriority>,
    task_description: Option<String>,
    estimated_duration: Option<chrono::Duration>,
    context_data: HashMap<String, String>,
}

impl WorkerSpawnRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requesting_agent_id(mut self, agent_id: Uuid) -> Self {
        self.requesting_agent_id = Some(agent_id);
        self
    }

    pub fn target_project<S: Into<String>>(mut self, project: S) -> Self {
        self.target_project = Some(project.into());
        self
    }

    pub fn required_capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_capabilities
            .extend(capabilities.into_iter().map(|c| c.into()));
        self
    }

    pub fn priority(mut self, priority: SpawnPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn task_description<S: Into<String>>(mut self, description: S) -> Self {
        self.task_description = Some(description.into());
        self
    }

    pub fn estimated_duration(mut self, duration: chrono::Duration) -> Self {
        self.estimated_duration = Some(duration);
        self
    }

    pub fn context_data<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.context_data.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Result<WorkerSpawnRequest> {
        let now = Utc::now();
        Ok(WorkerSpawnRequest {
            id: Uuid::new_v4(),
            requesting_agent_id: self
                .requesting_agent_id
                .ok_or_else(|| Error::validation("requesting_agent_id is required"))?,
            target_project: self
                .target_project
                .ok_or_else(|| Error::validation("target_project is required"))?,
            required_capabilities: self.required_capabilities,
            priority: self.priority.unwrap_or(SpawnPriority::Medium),
            task_description: self
                .task_description
                .ok_or_else(|| Error::validation("task_description is required"))?,
            estimated_duration: self.estimated_duration,
            context_data: self.context_data,
            status: SpawnRequestStatus::Pending,
            created_at: now,
            updated_at: now,
            assigned_worker_id: None,
            spawn_result: None,
        })
    }
}

#[derive(Debug, Default)]
pub struct WorkCoordinationAgreementBuilder {
    initiating_agent_id: Option<Uuid>,
    target_agent_id: Option<Uuid>,
    coordination_type: Option<WorkCoordinationType>,
    work_items: Vec<WorkItem>,
}

impl WorkCoordinationAgreementBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initiating_agent_id(mut self, agent_id: Uuid) -> Self {
        self.initiating_agent_id = Some(agent_id);
        self
    }

    pub fn target_agent_id(mut self, agent_id: Uuid) -> Self {
        self.target_agent_id = Some(agent_id);
        self
    }

    pub fn coordination_type(mut self, coord_type: WorkCoordinationType) -> Self {
        self.coordination_type = Some(coord_type);
        self
    }

    pub fn work_item(mut self, item: WorkItem) -> Self {
        self.work_items.push(item);
        self
    }

    pub fn build(self) -> Result<WorkCoordinationAgreement> {
        let now = Utc::now();
        Ok(WorkCoordinationAgreement {
            id: Uuid::new_v4(),
            initiating_agent_id: self
                .initiating_agent_id
                .ok_or_else(|| Error::validation("initiating_agent_id is required"))?,
            target_agent_id: self
                .target_agent_id
                .ok_or_else(|| Error::validation("target_agent_id is required"))?,
            coordination_type: self
                .coordination_type
                .ok_or_else(|| Error::validation("coordination_type is required"))?,
            work_items: self.work_items,
            dependencies: Vec::new(),
            negotiated_timeline: CoordinationTimeline {
                milestones: Vec::new(),
                estimated_completion: now + chrono::Duration::hours(24),
                critical_path: Vec::new(),
            },
            status: CoordinationStatus::Proposed,
            created_at: now,
            agreed_at: None,
            completed_at: None,
            terms: CoordinationTerms {
                resource_allocation: HashMap::new(),
                communication_protocol: CommunicationProtocol {
                    update_frequency: "hourly".to_string(),
                    notification_triggers: Vec::new(),
                    escalation_timeout: Some(chrono::Duration::hours(4)),
                },
                escalation_rules: Vec::new(),
                success_criteria: Vec::new(),
                failure_conditions: Vec::new(),
            },
        })
    }
}

#[derive(Debug, Default)]
pub struct ConflictResolutionCaseBuilder {
    affected_agents: Vec<Uuid>,
    conflicted_resources: Vec<String>,
    conflict_type: Option<ConflictType>,
}

impl ConflictResolutionCaseBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn affected_agent(mut self, agent_id: Uuid) -> Self {
        self.affected_agents.push(agent_id);
        self
    }

    pub fn conflicted_resource<S: Into<String>>(mut self, resource: S) -> Self {
        self.conflicted_resources.push(resource.into());
        self
    }

    pub fn conflict_type(mut self, conflict_type: ConflictType) -> Self {
        self.conflict_type = Some(conflict_type);
        self
    }

    pub fn build(self) -> Result<ConflictResolutionCase> {
        let now = Utc::now();
        Ok(ConflictResolutionCase {
            id: Uuid::new_v4(),
            affected_agents: self.affected_agents,
            conflicted_resources: self.conflicted_resources,
            conflict_type: self
                .conflict_type
                .ok_or_else(|| Error::validation("conflict_type is required"))?,
            resolution_strategy: None,
            resolver_agent_id: None,
            status: ConflictStatus::Detected,
            created_at: now,
            resolved_at: None,
            resolution_plan: None,
            evidence: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_project_dependency_builder() {
        let dependency = CrossProjectDependency::builder()
            .declaring_agent_id(Uuid::new_v4())
            .source_project("frontend-app")
            .target_project("api-server")
            .dependency_type(DependencyType::ApiChange)
            .description("Need API endpoint for user preferences")
            .impact(DependencyImpact::Blocker)
            .urgency(DependencyUrgency::High)
            .affected_file("src/api/client.rs")
            .metadata("priority", "high")
            .build()
            .unwrap();

        assert_eq!(dependency.source_project, "frontend-app");
        assert_eq!(dependency.target_project, "api-server");
        assert_eq!(dependency.dependency_type, DependencyType::ApiChange);
        assert_eq!(dependency.impact, DependencyImpact::Blocker);
        assert_eq!(dependency.urgency, DependencyUrgency::High);
        assert_eq!(dependency.status, DependencyStatus::Declared);
        assert_eq!(dependency.affected_files.len(), 1);
        assert_eq!(
            dependency.metadata.get("priority"),
            Some(&"high".to_string())
        );
    }

    #[test]
    fn test_worker_spawn_request_builder() {
        let request = WorkerSpawnRequest::builder()
            .requesting_agent_id(Uuid::new_v4())
            .target_project("api-server")
            .required_capabilities(vec!["backend-development", "api-design"])
            .priority(SpawnPriority::High)
            .task_description("Implement user preferences API endpoints")
            .context_data("repository", "https://github.com/example/api-server")
            .build()
            .unwrap();

        assert_eq!(request.target_project, "api-server");
        assert_eq!(request.required_capabilities.len(), 2);
        assert_eq!(request.priority, SpawnPriority::High);
        assert_eq!(request.status, SpawnRequestStatus::Pending);
        assert!(request.context_data.contains_key("repository"));
    }

    #[test]
    fn test_work_coordination_agreement_builder() {
        let agreement = WorkCoordinationAgreement::builder()
            .initiating_agent_id(Uuid::new_v4())
            .target_agent_id(Uuid::new_v4())
            .coordination_type(WorkCoordinationType::Sequential)
            .build()
            .unwrap();

        assert_eq!(
            agreement.coordination_type,
            WorkCoordinationType::Sequential
        );
        assert_eq!(agreement.status, CoordinationStatus::Proposed);
        assert_eq!(
            agreement.terms.communication_protocol.update_frequency,
            "hourly"
        );
    }

    #[test]
    fn test_conflict_resolution_case_builder() {
        let case = ConflictResolutionCase::builder()
            .affected_agent(Uuid::new_v4())
            .affected_agent(Uuid::new_v4())
            .conflicted_resource("src/shared/utils.rs")
            .conflict_type(ConflictType::FileModification)
            .build()
            .unwrap();

        assert_eq!(case.affected_agents.len(), 2);
        assert_eq!(case.conflicted_resources.len(), 1);
        assert_eq!(case.conflict_type, ConflictType::FileModification);
        assert_eq!(case.status, ConflictStatus::Detected);
    }
}
