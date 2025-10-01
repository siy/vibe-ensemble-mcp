use serde::{Deserialize, Serialize};

/// Specification for a ticket to be created by a planning worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketSpecification {
    /// Temporary ID for referencing this ticket in dependencies
    pub temp_id: String,
    /// Human-readable title for the ticket
    pub title: String,
    /// Detailed description of the work to be done
    pub description: String,
    /// Execution plan (list of stage names the ticket will go through)
    pub execution_plan: Vec<String>,
    /// Optional subsystem hint (e.g., "FE", "BE", "DB"). If not provided, will be inferred from stages.
    pub subsystem: Option<String>,
    /// Type of ticket (e.g., "task", "bug", "feature")
    #[serde(default)]
    pub ticket_type: Option<String>,
    /// Priority level (e.g., "low", "medium", "high", "critical")
    #[serde(default)]
    pub priority: Option<String>,
    /// List of temp_ids this ticket depends on
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Specification for a worker type to be created by a planning worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTypeSpecification {
    /// Name of the worker type (e.g., "frontend_implementation")
    pub worker_type: String,
    /// Template to use (e.g., "implementation", "review", "testing")
    pub template: String,
    /// Optional short description
    pub short_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
    pub ticket_id: Option<String>,
    pub outcome: WorkerOutcome,
    pub comment: String,
    pub reason: String,

    /// Planning-specific: tickets to create
    #[serde(default)]
    pub tickets_to_create: Vec<TicketSpecification>,

    /// Planning-specific: worker types needed
    #[serde(default)]
    pub worker_types_needed: Vec<WorkerTypeSpecification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    NextStage,
    PrevStage,
    CoordinatorAttention,
    /// Planning worker has completed and provided ticket specifications
    PlanningComplete,
}
