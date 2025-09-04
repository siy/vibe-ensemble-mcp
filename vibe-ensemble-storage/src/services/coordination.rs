//! Cross-project dependency coordination service
//!
//! This service provides sophisticated coordination logic for managing dependencies
//! and work between multiple Claude Code agents across different projects.

use crate::repositories::{AgentRepository, IssueRepository, MessageRepository, ProjectRepository};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::{
    agent::Agent,
    coordination::{
        ActionStatus, ActionType, ConflictResolutionCase, ConflictStatus, ConflictType,
        CoordinationPlan, CoordinationPlanType, CrossProjectDependency, DependencyImpact,
        DependencyStatus, DependencyType, DependencyUrgency, RequiredAction, ResolutionStrategy,
        SpawnPriority, SpawnRequestStatus, WorkCoordinationAgreement, WorkCoordinationType,
        WorkerSpawnRequest,
    },
    issue::{Issue, IssuePriority},
    message::{Message, MessagePriority, MessageType},
};

/// Service for coordinating cross-project dependencies and worker collaboration
#[derive(Clone)]
pub struct CoordinationService {
    agent_repo: Arc<AgentRepository>,
    #[allow(dead_code)]
    issue_repo: Arc<IssueRepository>,
    message_repo: Arc<MessageRepository>,
    project_repo: Arc<ProjectRepository>,
}

impl CoordinationService {
    /// Create a new coordination service
    pub fn new(
        agent_repo: Arc<AgentRepository>,
        issue_repo: Arc<IssueRepository>,
        message_repo: Arc<MessageRepository>,
        project_repo: Arc<ProjectRepository>,
    ) -> Self {
        Self {
            agent_repo,
            issue_repo,
            message_repo,
            project_repo,
        }
    }

    /// Declare a cross-project dependency and create coordination plan
    #[allow(clippy::too_many_arguments)]
    pub async fn declare_dependency(
        &self,
        declaring_agent_id: Uuid,
        source_project: Uuid,
        target_project: Uuid,
        dependency_type: DependencyType,
        description: String,
        impact: DependencyImpact,
        urgency: DependencyUrgency,
        affected_files: Vec<String>,
        metadata: HashMap<String, String>,
    ) -> Result<(CrossProjectDependency, CoordinationPlan, Option<Issue>)> {
        info!(
            "Declaring cross-project dependency from {} to {} by agent {}",
            source_project, target_project, declaring_agent_id
        );

        // Verify declaring agent exists
        let declaring_agent = self
            .agent_repo
            .find_by_id(declaring_agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: declaring_agent_id.to_string(),
            })?;

        // Create dependency record
        let mut dependency = CrossProjectDependency::builder()
            .declaring_agent_id(declaring_agent_id)
            .source_project(source_project)
            .target_project(target_project)
            .dependency_type(dependency_type.clone())
            .description(description.clone())
            .impact(impact.clone())
            .urgency(urgency.clone())
            .affected_files(affected_files.clone())
            .build()?;

        for (key, value) in metadata {
            dependency.metadata.insert(key, value);
        }

        // Detect active workers on target project
        let target_workers = self.find_active_workers_for_project(target_project).await?;
        debug!(
            "Found {} active workers on target project {}",
            target_workers.len(),
            target_project
        );

        // Create coordination plan based on worker availability
        let plan_type = if target_workers.is_empty() {
            CoordinationPlanType::WorkerSpawn
        } else {
            CoordinationPlanType::DirectCoordination
        };

        let coordination_plan = self
            .create_coordination_plan(&dependency, plan_type, &target_workers)
            .await?;

        dependency.coordination_plan = Some(coordination_plan.clone());
        dependency.status = DependencyStatus::Planned;

        // Create tracking issue
        let issue = self
            .create_dependency_tracking_issue(&dependency, &declaring_agent)
            .await?;

        // Notify relevant agents
        self.notify_agents_of_dependency(&dependency, &target_workers)
            .await?;

        Ok((dependency, coordination_plan, Some(issue)))
    }

    /// Request coordinator to spawn a new worker for a project
    #[allow(clippy::too_many_arguments)]
    pub async fn request_worker_spawn(
        &self,
        requesting_agent_id: Uuid,
        target_project: Uuid,
        required_capabilities: Vec<String>,
        priority: SpawnPriority,
        task_description: String,
        estimated_duration: Option<chrono::Duration>,
        context_data: HashMap<String, String>,
    ) -> Result<WorkerSpawnRequest> {
        info!(
            "Requesting worker spawn for project {} by agent {}",
            target_project, requesting_agent_id
        );

        // Verify requesting agent exists
        let _requesting_agent = self
            .agent_repo
            .find_by_id(requesting_agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: requesting_agent_id.to_string(),
            })?;

        // Check if workers are already available
        let existing_workers = self.find_active_workers_for_project(target_project).await?;
        let capability_matches = self
            .find_workers_with_capabilities(&required_capabilities)
            .await?;

        // Create spawn request
        let mut spawn_request = WorkerSpawnRequest::builder()
            .requesting_agent_id(requesting_agent_id)
            .target_project(target_project)
            .required_capabilities(required_capabilities.clone())
            .priority(priority)
            .task_description(task_description.clone())
            .build()?;

        if let Some(duration) = estimated_duration {
            spawn_request.estimated_duration = Some(duration);
        }

        spawn_request.context_data = context_data;

        // Evaluate spawn request
        spawn_request.status = if existing_workers.is_empty() && capability_matches.is_empty() {
            SpawnRequestStatus::Approved
        } else {
            SpawnRequestStatus::Evaluating
        };

        // Create issue for spawn request tracking
        let _tracking_issue = self
            .create_spawn_request_tracking_issue(&spawn_request)
            .await?;

        Ok(spawn_request)
    }

    /// Coordinate work between agents
    pub async fn coordinate_work(
        &self,
        initiating_agent_id: Uuid,
        target_agent_id: Uuid,
        coordination_type: WorkCoordinationType,
        work_items: Vec<vibe_ensemble_core::coordination::WorkItem>,
        dependencies: Vec<vibe_ensemble_core::coordination::WorkDependency>,
        proposed_timeline: Option<vibe_ensemble_core::coordination::CoordinationTimeline>,
    ) -> Result<WorkCoordinationAgreement> {
        info!(
            "Coordinating work between agents {} and {}",
            initiating_agent_id, target_agent_id
        );

        // Verify both agents exist and are available
        let initiating_agent = self
            .agent_repo
            .find_by_id(initiating_agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: initiating_agent_id.to_string(),
            })?;

        let target_agent = self
            .agent_repo
            .find_by_id(target_agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: target_agent_id.to_string(),
            })?;

        if !initiating_agent.is_available() {
            return Err(Error::ConstraintViolation(
                "Initiating agent is not available for coordination".to_string(),
            ));
        }

        if !target_agent.is_available() {
            return Err(Error::ConstraintViolation(
                "Target agent is not available for coordination".to_string(),
            ));
        }

        // Create coordination agreement
        let mut agreement = WorkCoordinationAgreement::builder()
            .initiating_agent_id(initiating_agent_id)
            .target_agent_id(target_agent_id)
            .coordination_type(coordination_type)
            .build()?;

        agreement.work_items = work_items;
        agreement.dependencies = dependencies;

        if let Some(timeline) = proposed_timeline {
            agreement.negotiated_timeline = timeline;
        }

        // Send coordination proposal to target agent
        self.send_coordination_proposal(&agreement).await?;

        // Create tracking issue
        let _tracking_issue = self.create_coordination_tracking_issue(&agreement).await?;

        Ok(agreement)
    }

    /// Resolve conflicts between agents
    pub async fn resolve_conflict(
        &self,
        affected_agents: Vec<Uuid>,
        conflicted_resources: Vec<String>,
        conflict_type: ConflictType,
        resolution_strategy: Option<ResolutionStrategy>,
        resolver_agent_id: Uuid,
    ) -> Result<ConflictResolutionCase> {
        info!(
            "Resolving conflict involving {} agents and {} resources",
            affected_agents.len(),
            conflicted_resources.len()
        );

        // Verify resolver agent exists
        let _resolver_agent = self
            .agent_repo
            .find_by_id(resolver_agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: resolver_agent_id.to_string(),
            })?;

        // Verify all affected agents exist
        for agent_id in &affected_agents {
            self.agent_repo
                .find_by_id(*agent_id)
                .await?
                .ok_or_else(|| Error::NotFound {
                    entity: "Agent".to_string(),
                    id: agent_id.to_string(),
                })?;
        }

        // Create conflict resolution case
        let mut conflict_case = ConflictResolutionCase::builder()
            .conflict_type(conflict_type.clone())
            .build()?;

        conflict_case.affected_agents = affected_agents.clone();
        conflict_case.conflicted_resources = conflicted_resources.clone();
        conflict_case.resolver_agent_id = Some(resolver_agent_id);

        // Determine resolution strategy if not provided
        let strategy = resolution_strategy.unwrap_or_else(|| {
            self.determine_resolution_strategy(&conflict_type, &conflicted_resources)
        });

        conflict_case.resolution_strategy = Some(strategy.clone());
        conflict_case.status = ConflictStatus::ResolutionPlanned;

        // Create resolution plan
        let resolution_plan = self
            .create_conflict_resolution_plan(&conflict_case, strategy)
            .await?;

        conflict_case.resolution_plan = Some(resolution_plan);

        // Notify affected agents
        self.notify_agents_of_conflict(&conflict_case).await?;

        // Create tracking issue
        let _tracking_issue = self.create_conflict_tracking_issue(&conflict_case).await?;

        Ok(conflict_case)
    }

    // Private helper methods

    /// Find active workers for a specific project
    async fn find_active_workers_for_project(&self, _project: Uuid) -> Result<Vec<Agent>> {
        let all_agents = self.agent_repo.list().await?;
        let active_workers: Vec<Agent> = all_agents
            .into_iter()
            .filter(|agent| {
                agent.is_available()
                // TODO: Add project tracking mechanism to agents
                // For now, return all available agents
            })
            .collect();

        Ok(active_workers)
    }

    /// Find workers with specific capabilities
    async fn find_workers_with_capabilities(&self, capabilities: &[String]) -> Result<Vec<Agent>> {
        let all_agents = self.agent_repo.list().await?;
        let matching_workers: Vec<Agent> = all_agents
            .into_iter()
            .filter(|agent| agent.is_available() && agent.has_all_capabilities(capabilities))
            .collect();

        Ok(matching_workers)
    }

    /// Create coordination plan for resolving dependency
    async fn create_coordination_plan(
        &self,
        dependency: &CrossProjectDependency,
        plan_type: CoordinationPlanType,
        target_workers: &[Agent],
    ) -> Result<CoordinationPlan> {
        let mut required_actions = Vec::new();
        let estimated_duration = self.estimate_coordination_duration(dependency, &plan_type);

        match plan_type {
            CoordinationPlanType::DirectCoordination => {
                // Create actions for direct coordination with existing workers
                for worker in target_workers {
                    let action = RequiredAction {
                        id: Uuid::new_v4(),
                        action_type: ActionType::Coordinate,
                        description: format!(
                            "Coordinate with agent {} on {} changes",
                            worker.name, dependency.target_project
                        ),
                        assigned_agent_id: Some(worker.id),
                        target_project: dependency.target_project,
                        affected_resources: dependency.affected_files.clone(),
                        estimated_effort: Some("2-4 hours".to_string()),
                        dependencies: Vec::new(),
                        status: ActionStatus::Pending,
                        created_at: chrono::Utc::now(),
                        completed_at: None,
                    };
                    required_actions.push(action);
                }
            }
            CoordinationPlanType::WorkerSpawn => {
                // Create action for requesting worker spawn
                let action = RequiredAction {
                    id: Uuid::new_v4(),
                    action_type: ActionType::Coordinate,
                    description: format!(
                        "Request coordinator to spawn worker for {}",
                        dependency.target_project
                    ),
                    assigned_agent_id: Some(dependency.declaring_agent_id),
                    target_project: dependency.target_project,
                    affected_resources: dependency.affected_files.clone(),
                    estimated_effort: Some("1-2 hours".to_string()),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                };
                required_actions.push(action);
            }
            _ => {
                // Default coordination actions
                let action = RequiredAction {
                    id: Uuid::new_v4(),
                    action_type: ActionType::Implement,
                    description: format!(
                        "Resolve {:?} dependency in {}",
                        dependency.dependency_type, dependency.target_project
                    ),
                    assigned_agent_id: None,
                    target_project: dependency.target_project,
                    affected_resources: dependency.affected_files.clone(),
                    estimated_effort: Some("4-8 hours".to_string()),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                };
                required_actions.push(action);
            }
        }

        let assigned_agents = target_workers.iter().map(|w| w.id).collect();

        Ok(CoordinationPlan {
            id: Uuid::new_v4(),
            dependency_id: dependency.id,
            plan_type,
            required_actions,
            estimated_duration: Some(estimated_duration),
            assigned_agents,
            created_at: chrono::Utc::now(),
            timeline: None,
        })
    }

    /// Estimate duration for coordination based on dependency and plan type
    fn estimate_coordination_duration(
        &self,
        dependency: &CrossProjectDependency,
        plan_type: &CoordinationPlanType,
    ) -> chrono::Duration {
        let base_duration = match dependency.impact {
            DependencyImpact::Blocker => chrono::Duration::hours(8),
            DependencyImpact::Major => chrono::Duration::hours(4),
            DependencyImpact::Minor => chrono::Duration::hours(2),
            DependencyImpact::Info => chrono::Duration::hours(1),
        };

        let multiplier = match plan_type {
            CoordinationPlanType::DirectCoordination => 1.0,
            CoordinationPlanType::WorkerSpawn => 2.0,
            CoordinationPlanType::Sequential => 1.5,
            CoordinationPlanType::Parallel => 0.8,
            CoordinationPlanType::ConflictResolution => 3.0,
        };

        let urgency_multiplier = match dependency.urgency {
            DependencyUrgency::Critical => 0.5,
            DependencyUrgency::High => 0.7,
            DependencyUrgency::Medium => 1.0,
            DependencyUrgency::Low => 1.5,
        };

        chrono::Duration::milliseconds(
            (base_duration.num_milliseconds() as f64 * multiplier * urgency_multiplier) as i64,
        )
    }

    /// Create tracking issue for dependency
    async fn create_dependency_tracking_issue(
        &self,
        dependency: &CrossProjectDependency,
        declaring_agent: &Agent,
    ) -> Result<Issue> {
        let priority = match dependency.urgency {
            DependencyUrgency::Critical => IssuePriority::Critical,
            DependencyUrgency::High => IssuePriority::High,
            DependencyUrgency::Medium => IssuePriority::Medium,
            DependencyUrgency::Low => IssuePriority::Low,
        };

        let title = format!(
            "[DEPENDENCY] {} → {}: {}",
            dependency.source_project,
            dependency.target_project,
            dependency.description.chars().take(50).collect::<String>()
        );

        let description = format!(
            "Cross-project dependency declared by agent {}\n\n\
            Source Project: {}\n\
            Target Project: {}\n\
            Dependency Type: {:?}\n\
            Impact: {:?}\n\
            Urgency: {:?}\n\n\
            Description: {}\n\n\
            Affected Files:\n{}\n\n\
            Coordination Plan: {}",
            declaring_agent.name,
            dependency.source_project,
            dependency.target_project,
            dependency.dependency_type,
            dependency.impact,
            dependency.urgency,
            dependency.description,
            dependency
                .affected_files
                .iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n"),
            dependency
                .coordination_plan
                .as_ref()
                .map_or("None".to_string(), |p| format!("{:?}", p.plan_type))
        );

        let mut issue = Issue::builder()
            .title(title)
            .description(description)
            .priority(priority)
            .tag("cross-project-dependency")
            .tag(dependency.source_project.to_string())
            .tag(dependency.target_project.to_string())
            .build()?;

        // Link to dependency metadata
        issue
            .knowledge_links
            .push(format!("dependency:{}", dependency.id));

        Ok(issue)
    }

    /// Create tracking issue for worker spawn request
    async fn create_spawn_request_tracking_issue(
        &self,
        spawn_request: &WorkerSpawnRequest,
    ) -> Result<Issue> {
        let priority = match spawn_request.priority {
            SpawnPriority::Critical => IssuePriority::Critical,
            SpawnPriority::High => IssuePriority::High,
            SpawnPriority::Medium => IssuePriority::Medium,
            SpawnPriority::Low => IssuePriority::Low,
        };

        let title = format!(
            "[SPAWN REQUEST] Worker needed for {}: {}",
            spawn_request.target_project,
            spawn_request
                .task_description
                .chars()
                .take(50)
                .collect::<String>()
        );

        let description = format!(
            "Worker spawn request\n\n\
            Target Project: {}\n\
            Required Capabilities: {}\n\
            Priority: {:?}\n\
            Estimated Duration: {}\n\n\
            Task Description: {}\n\n\
            Context Data: {}",
            spawn_request.target_project,
            spawn_request.required_capabilities.join(", "),
            spawn_request.priority,
            spawn_request
                .estimated_duration
                .map_or("Unknown".to_string(), |d| format!(
                    "{} minutes",
                    d.num_minutes()
                )),
            spawn_request.task_description,
            serde_json::to_string_pretty(&spawn_request.context_data).unwrap_or_default()
        );

        let issue = Issue::builder()
            .title(title)
            .description(description)
            .priority(priority)
            .tag("worker-spawn-request")
            .tag(spawn_request.target_project.to_string())
            .build()?;

        Ok(issue)
    }

    /// Create tracking issue for work coordination
    async fn create_coordination_tracking_issue(
        &self,
        agreement: &WorkCoordinationAgreement,
    ) -> Result<Issue> {
        let title = format!(
            "[COORDINATION] {} work between agents",
            match agreement.coordination_type {
                WorkCoordinationType::Sequential => "Sequential",
                WorkCoordinationType::Parallel => "Parallel",
                WorkCoordinationType::Blocking => "Blocking",
                WorkCoordinationType::Collaborative => "Collaborative",
                WorkCoordinationType::ConflictResolution => "Conflict Resolution",
            }
        );

        let description = format!(
            "Work coordination agreement\n\n\
            Coordination Type: {:?}\n\
            Initiating Agent: {}\n\
            Target Agent: {}\n\
            Work Items: {}\n\n\
            Status: {:?}",
            agreement.coordination_type,
            agreement.initiating_agent_id,
            agreement.target_agent_id,
            agreement.work_items.len(),
            agreement.status
        );

        let issue = Issue::builder()
            .title(title)
            .description(description)
            .priority(IssuePriority::Medium)
            .tag("work-coordination")
            .build()?;

        Ok(issue)
    }

    /// Create tracking issue for conflict resolution
    async fn create_conflict_tracking_issue(
        &self,
        conflict_case: &ConflictResolutionCase,
    ) -> Result<Issue> {
        let title = format!(
            "[CONFLICT] {:?} conflict affecting {} agents",
            conflict_case.conflict_type,
            conflict_case.affected_agents.len()
        );

        let description = format!(
            "Conflict resolution case\n\n\
            Conflict Type: {:?}\n\
            Affected Agents: {}\n\
            Conflicted Resources: {}\n\
            Resolution Strategy: {:?}\n\n\
            Status: {:?}",
            conflict_case.conflict_type,
            conflict_case.affected_agents.len(),
            conflict_case.conflicted_resources.join(", "),
            conflict_case.resolution_strategy,
            conflict_case.status
        );

        let issue = Issue::builder()
            .title(title)
            .description(description)
            .priority(IssuePriority::High)
            .tag("conflict-resolution")
            .build()?;

        Ok(issue)
    }

    /// Notify agents about new dependency
    async fn notify_agents_of_dependency(
        &self,
        dependency: &CrossProjectDependency,
        target_workers: &[Agent],
    ) -> Result<()> {
        for worker in target_workers {
            let message = Message::builder()
                .sender_id(dependency.declaring_agent_id)
                .recipient_id(worker.id)
                .message_type(MessageType::IssueNotification)
                .content(format!(
                    "New cross-project dependency declared: {} requires changes in {}. Impact: {:?}, Urgency: {:?}",
                    dependency.source_project,
                    dependency.target_project,
                    dependency.impact,
                    dependency.urgency
                ))
                .priority(match dependency.urgency {
                    DependencyUrgency::Critical => MessagePriority::Urgent,
                    DependencyUrgency::High => MessagePriority::High,
                    DependencyUrgency::Medium => MessagePriority::Normal,
                    DependencyUrgency::Low => MessagePriority::Low,
                })
                .build()?;

            if let Err(e) = self.message_repo.create(&message).await {
                warn!(
                    "Failed to send dependency notification to agent {}: {}",
                    worker.id, e
                );
            }
        }

        Ok(())
    }

    /// Send coordination proposal to target agent
    async fn send_coordination_proposal(
        &self,
        agreement: &WorkCoordinationAgreement,
    ) -> Result<()> {
        let message = Message::builder()
            .sender_id(agreement.initiating_agent_id)
            .recipient_id(agreement.target_agent_id)
            .message_type(MessageType::Direct)
            .content(format!(
                "Work coordination proposal: {:?} coordination with {} work items",
                agreement.coordination_type,
                agreement.work_items.len()
            ))
            .priority(MessagePriority::Normal)
            .build()?;

        self.message_repo.create(&message).await?;
        Ok(())
    }

    /// Notify agents about conflict
    async fn notify_agents_of_conflict(
        &self,
        conflict_case: &ConflictResolutionCase,
    ) -> Result<()> {
        for &agent_id in &conflict_case.affected_agents {
            let message = Message::builder()
                .sender_id(conflict_case.resolver_agent_id.unwrap_or(Uuid::new_v4()))
                .recipient_id(agent_id)
                .message_type(MessageType::IssueNotification)
                .content(format!(
                    "Conflict detected: {:?} conflict on resources: {}",
                    conflict_case.conflict_type,
                    conflict_case.conflicted_resources.join(", ")
                ))
                .priority(MessagePriority::High)
                .build()?;

            if let Err(e) = self.message_repo.create(&message).await {
                warn!(
                    "Failed to send conflict notification to agent {}: {}",
                    agent_id, e
                );
            }
        }

        Ok(())
    }

    /// Determine appropriate resolution strategy for conflict type
    fn determine_resolution_strategy(
        &self,
        conflict_type: &ConflictType,
        _conflicted_resources: &[String],
    ) -> ResolutionStrategy {
        match conflict_type {
            ConflictType::FileModification => ResolutionStrategy::ManualMerge,
            ConflictType::ResourceLock => ResolutionStrategy::Sequential,
            ConflictType::Architecture => ResolutionStrategy::Escalate,
            ConflictType::BusinessLogic => ResolutionStrategy::ManualMerge,
            ConflictType::Testing => ResolutionStrategy::AutoMerge,
            ConflictType::Deployment => ResolutionStrategy::Sequential,
        }
    }

    /// Create detailed resolution plan for conflict
    async fn create_conflict_resolution_plan(
        &self,
        conflict_case: &ConflictResolutionCase,
        strategy: ResolutionStrategy,
    ) -> Result<vibe_ensemble_core::coordination::ConflictResolutionPlan> {
        let steps = match strategy {
            ResolutionStrategy::ManualMerge => vec![
                vibe_ensemble_core::coordination::ResolutionStep {
                    id: Uuid::new_v4(),
                    sequence: 1,
                    description: "Analyze conflicting changes".to_string(),
                    assigned_agent_id: conflict_case.resolver_agent_id,
                    estimated_duration: Some(chrono::Duration::hours(1)),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                },
                vibe_ensemble_core::coordination::ResolutionStep {
                    id: Uuid::new_v4(),
                    sequence: 2,
                    description: "Create merge strategy".to_string(),
                    assigned_agent_id: conflict_case.resolver_agent_id,
                    estimated_duration: Some(chrono::Duration::hours(2)),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                },
            ],
            ResolutionStrategy::Sequential => {
                vec![vibe_ensemble_core::coordination::ResolutionStep {
                    id: Uuid::new_v4(),
                    sequence: 1,
                    description: "Establish work ordering".to_string(),
                    assigned_agent_id: conflict_case.resolver_agent_id,
                    estimated_duration: Some(chrono::Duration::minutes(30)),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                }]
            }
            _ => vec![vibe_ensemble_core::coordination::ResolutionStep {
                id: Uuid::new_v4(),
                sequence: 1,
                description: format!("Apply {:?} resolution strategy", strategy),
                assigned_agent_id: conflict_case.resolver_agent_id,
                estimated_duration: Some(chrono::Duration::hours(1)),
                dependencies: Vec::new(),
                status: ActionStatus::Pending,
            }],
        };

        let required_actions_per_agent = conflict_case
            .affected_agents
            .iter()
            .map(|&agent_id| {
                let actions = vec![RequiredAction {
                    id: Uuid::new_v4(),
                    action_type: ActionType::Coordinate,
                    description: "Participate in conflict resolution".to_string(),
                    assigned_agent_id: Some(agent_id),
                    target_project: Uuid::new_v4(), // Mock project ID for conflict resolution
                    affected_resources: conflict_case.conflicted_resources.clone(),
                    estimated_effort: Some("1-2 hours".to_string()),
                    dependencies: Vec::new(),
                    status: ActionStatus::Pending,
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                }];
                (agent_id, actions)
            })
            .collect();

        Ok(vibe_ensemble_core::coordination::ConflictResolutionPlan {
            id: Uuid::new_v4(),
            conflict_id: conflict_case.id,
            strategy,
            steps,
            required_actions_per_agent,
            estimated_resolution_time: Some(chrono::Duration::hours(4)),
            rollback_plan: None, // TODO: Implement rollback plans
        })
    }

    /// Validate cross-project dependency with project entity validation
    pub async fn validate_cross_project_dependency(
        &self,
        source_project: &Uuid,
        target_project: &Uuid,
        declaring_agent_id: &Uuid,
    ) -> Result<ProjectValidationResult> {
        debug!(
            "Validating cross-project dependency: {} -> {} (agent: {})",
            source_project, target_project, declaring_agent_id
        );

        let mut validation = ProjectValidationResult {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        // Validate source project exists
        let source_exists = self.project_repo.find_by_id(source_project).await?;
        if source_exists.is_none() {
            validation.valid = false;
            validation
                .errors
                .push(format!("Source project {} not found", source_project));
        }

        // Validate target project exists
        let target_exists = self.project_repo.find_by_id(target_project).await?;
        if target_exists.is_none() {
            validation.valid = false;
            validation
                .errors
                .push(format!("Target project {} not found", target_project));
        }

        // Validate declaring agent exists
        let agent_exists = self.agent_repo.find_by_id(*declaring_agent_id).await?;
        if agent_exists.is_none() {
            validation.valid = false;
            validation
                .errors
                .push(format!("Declaring agent {} not found", declaring_agent_id));
        }

        // Check if agent is assigned to source project
        if let Some(agent) = agent_exists {
            if agent.connection_metadata.project_id != Some(*source_project) {
                // TODO: Consider upgrading to error for stricter enforcement
                validation.warnings.push(format!(
                    "Agent {} is not assigned to source project {} - this may warrant a hard error",
                    declaring_agent_id, source_project
                ));
            }
        }

        // Warn if source and target are the same project
        if source_project == target_project {
            validation
                .warnings
                .push("Cross-project dependency declared within same project".to_string());
        }

        Ok(validation)
    }

    /// Get coordination status for a specific project
    pub async fn get_project_coordination_status(
        &self,
        project_id: &Uuid,
    ) -> Result<ProjectCoordinationStatus> {
        debug!("Getting coordination status for project: {}", project_id);

        // Validate project exists
        let project = self
            .project_repo
            .find_by_id(project_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Project".to_string(),
                id: project_id.to_string(),
            })?;

        // Get agents assigned to this project
        let agents = self.agent_repo.find_by_project(project_id).await?;

        // For a full implementation, you'd also track dependencies and coordination activities
        // For now, provide basic status based on agent assignments
        let coordinator_count = agents
            .iter()
            .filter(|a| {
                matches!(
                    a.agent_type,
                    vibe_ensemble_core::agent::AgentType::Coordinator
                )
            })
            .count();

        let worker_count = agents
            .iter()
            .filter(|a| matches!(a.agent_type, vibe_ensemble_core::agent::AgentType::Worker))
            .count();

        Ok(ProjectCoordinationStatus {
            project_id: *project_id,
            project_name: project.name,
            active_agents: agents.len(),
            coordinator_agents: coordinator_count,
            worker_agents: worker_count,
            active_dependencies: 0,   // TODO: Wire dependency tracking system
            pending_coordinations: 0, // TODO: Wire coordination activity tracking
            last_activity: project.updated_at,
        })
    }

    /// Validate agent can participate in cross-project coordination
    pub async fn validate_agent_coordination_eligibility(
        &self,
        agent_id: &Uuid,
        target_project: &Uuid,
    ) -> Result<AgentCoordinationEligibility> {
        debug!(
            "Validating agent {} eligibility for project {}",
            agent_id, target_project
        );

        let agent = self
            .agent_repo
            .find_by_id(*agent_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Agent".to_string(),
                id: agent_id.to_string(),
            })?;

        let target_project_exists = self.project_repo.find_by_id(target_project).await?;
        if target_project_exists.is_none() {
            return Ok(AgentCoordinationEligibility {
                eligible: false,
                agent_id: *agent_id,
                reason: "Target project does not exist".to_string(),
                recommendations: vec!["Verify project ID is correct".to_string()],
            });
        }

        // Check agent availability (accounts for load and status)
        let eligible = agent.is_available();
        let reason = if eligible {
            "Agent is available for cross-project coordination".to_string()
        } else {
            "Agent is not available".to_string()
        };

        let mut recommendations = Vec::new();
        if agent.connection_metadata.project_id.is_none() {
            recommendations.push("Consider assigning agent to a primary project".to_string());
        }
        if !agent.has_capability("coordination") {
            recommendations
                .push("Agent may benefit from coordination capability training".to_string());
        }

        Ok(AgentCoordinationEligibility {
            eligible,
            agent_id: *agent_id,
            reason,
            recommendations,
        })
    }
}

/// Result of project validation for coordination
#[derive(Debug, Clone)]
pub struct ProjectValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Coordination status for a specific project
#[derive(Debug, Clone)]
pub struct ProjectCoordinationStatus {
    pub project_id: Uuid,
    pub project_name: String,
    pub active_agents: usize,
    pub coordinator_agents: usize,
    pub worker_agents: usize,
    pub active_dependencies: usize,
    pub pending_coordinations: usize,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// Agent eligibility for cross-project coordination
#[derive(Debug, Clone)]
pub struct AgentCoordinationEligibility {
    pub eligible: bool,
    pub agent_id: Uuid,
    pub reason: String,
    pub recommendations: Vec<String>,
}

// TODO: Add comprehensive tests once in-memory repository implementations are available
// #[cfg(test)]
// mod tests {
//     use super::*;
//     // Tests would go here
// }
