//! Workflow coordination and task distribution
//!
//! This module provides functionality for coordinating multi-step workflows
//! across multiple Claude Code agents. It supports sequential and parallel
//! task execution with error handling, retries, and progress tracking.

use crate::orchestration::executor::{ExecutionResult, HeadlessClaudeExecutor};
use crate::orchestration::models::{
    WorkflowExecutionContext, WorkflowStats, WorkflowStatus, WorkspaceConfiguration,
};
use crate::orchestration::template_manager::TemplateManager;
use crate::prompt::{AgentTemplate, WorkflowStep};
use crate::{Error, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Maximum time to wait for workflow completion (seconds)
    pub timeout_seconds: u64,
    /// Whether to continue execution if a step fails
    pub continue_on_failure: bool,
    /// Number of retry attempts for failed steps
    pub max_retries: u32,
    /// Delay between retry attempts (seconds)
    pub retry_delay_seconds: u64,
    /// Whether to execute compatible steps in parallel
    pub enable_parallelism: bool,
    /// Maximum number of parallel executions
    pub max_parallel_executions: u32,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 1800, // 30 minutes
            continue_on_failure: false,
            max_retries: 2,
            retry_delay_seconds: 5,
            enable_parallelism: false,
            max_parallel_executions: 4,
        }
    }
}

/// A workflow step execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResult {
    /// Step identifier
    pub step_id: String,
    /// Whether the step was successful
    pub success: bool,
    /// Execution result from Claude Code
    pub execution_result: Option<ExecutionResult>,
    /// Error message if failed
    pub error: Option<String>,
    /// Number of retry attempts made
    pub retry_count: u32,
    /// Duration of step execution in milliseconds
    pub duration_ms: u64,
    /// When the step started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the step completed
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Context for a single workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Template defining the workflow
    pub template: AgentTemplate,
    /// Workspace for execution
    pub workspace: WorkspaceConfiguration,
    /// Variables available to all steps
    pub variables: HashMap<String, String>,
    /// Step execution configuration
    pub config: WorkflowConfig,
}

/// Result of workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// Workflow execution context
    pub workflow_id: Uuid,
    /// Final status
    pub status: WorkflowStatus,
    /// Results from individual steps
    pub step_results: HashMap<String, StepExecutionResult>,
    /// Overall execution statistics
    pub stats: WorkflowStats,
    /// When the workflow started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the workflow completed
    pub completed_at: chrono::DateTime<chrono::Utc>,
    /// Any workflow-level error
    pub error: Option<String>,
}

/// Workflow executor for orchestrating multi-step agent tasks
pub struct WorkflowExecutor {
    /// Claude Code executor
    executor: HeadlessClaudeExecutor,
    /// Template manager for loading agent configurations
    template_manager: Arc<dyn TemplateManager + Send + Sync>,
    /// Active workflow executions
    active_workflows: Arc<RwLock<HashMap<Uuid, WorkflowExecutionContext>>>,
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(
        executor: HeadlessClaudeExecutor,
        template_manager: Arc<dyn TemplateManager + Send + Sync>,
    ) -> Self {
        Self {
            executor,
            template_manager,
            active_workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a workflow from an agent template
    pub async fn execute_workflow(&self, context: WorkflowContext) -> Result<WorkflowResult> {
        let workflow_id = Uuid::new_v4();
        let started_at = Utc::now();

        // Initialize workflow context
        let execution_context = WorkflowExecutionContext {
            workflow_id,
            current_step: String::new(),
            variables: context.variables.clone(),
            step_results: HashMap::new(),
            metadata: HashMap::new(),
            started_at,
            status: WorkflowStatus::Running,
        };

        // Register active workflow
        self.active_workflows
            .write()
            .await
            .insert(workflow_id, execution_context.clone());

        // Execute workflow with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(context.config.timeout_seconds),
            self.execute_workflow_steps(&context, workflow_id),
        )
        .await;

        // Remove from active workflows
        self.active_workflows.write().await.remove(&workflow_id);

        let completed_at = Utc::now();

        match result {
            Ok(Ok((step_results, status, error))) => {
                let stats = self.calculate_workflow_stats(&step_results, started_at, completed_at);
                Ok(WorkflowResult {
                    workflow_id,
                    status,
                    step_results,
                    stats,
                    started_at,
                    completed_at,
                    error,
                })
            }
            Ok(Err(e)) => Ok(WorkflowResult {
                workflow_id,
                status: WorkflowStatus::Failed,
                step_results: HashMap::new(),
                stats: WorkflowStats {
                    total_duration_ms: (completed_at - started_at).num_milliseconds() as u64,
                    steps_executed: 0,
                    total_retries: 0,
                    peak_memory_mb: None,
                    total_cost: None,
                },
                started_at,
                completed_at,
                error: Some(e.to_string()),
            }),
            Err(_) => Ok(WorkflowResult {
                workflow_id,
                status: WorkflowStatus::Failed,
                step_results: HashMap::new(),
                stats: WorkflowStats {
                    total_duration_ms: (completed_at - started_at).num_milliseconds() as u64,
                    steps_executed: 0,
                    total_retries: 0,
                    peak_memory_mb: None,
                    total_cost: None,
                },
                started_at,
                completed_at,
                error: Some("Workflow execution timed out".to_string()),
            }),
        }
    }

    /// Execute all workflow steps
    async fn execute_workflow_steps(
        &self,
        context: &WorkflowContext,
        workflow_id: Uuid,
    ) -> Result<(
        HashMap<String, StepExecutionResult>,
        WorkflowStatus,
        Option<String>,
    )> {
        let mut step_results = HashMap::new();
        let mut workflow_variables = context.variables.clone();
        let mut workflow_error = None;

        // Get steps sorted by order
        let mut steps = context.template.workflow_steps.clone();
        steps.sort_by(|a, b| a.order.cmp(&b.order));

        for step in &steps {
            // Update current step in context
            if let Some(ctx) = self.active_workflows.write().await.get_mut(&workflow_id) {
                ctx.current_step = step.id.clone();
            }

            // Check if step should be executed based on conditions
            if !self.should_execute_step(step, &step_results, &workflow_variables) {
                continue;
            }

            // Execute step with retries
            let step_result = self
                .execute_step_with_retries(context, step, &workflow_variables)
                .await;

            let step_success = step_result.success;

            // Update workflow variables with step results
            if let Some(execution_result) = &step_result.execution_result {
                workflow_variables.insert(
                    format!("{}_output", step.id),
                    execution_result.content.clone(),
                );
                workflow_variables.insert(format!("{}_success", step.id), step_success.to_string());
            }

            step_results.insert(step.id.clone(), step_result);

            // Handle step failure
            if !step_success {
                if context.config.continue_on_failure {
                    workflow_error =
                        Some(format!("Step '{}' failed but continuing workflow", step.id));
                } else {
                    workflow_error = Some(format!("Step '{}' failed, stopping workflow", step.id));
                    return Ok((step_results, WorkflowStatus::Failed, workflow_error));
                }
            }
        }

        let final_status = if workflow_error.is_some() {
            WorkflowStatus::Failed
        } else {
            WorkflowStatus::Completed
        };

        Ok((step_results, final_status, workflow_error))
    }

    /// Execute a single step with retry logic
    async fn execute_step_with_retries(
        &self,
        context: &WorkflowContext,
        step: &WorkflowStep,
        variables: &HashMap<String, String>,
    ) -> StepExecutionResult {
        let started_at = Utc::now();
        let mut retry_count = 0;

        loop {
            let attempt_start = Utc::now();

            // Generate prompt for this step
            let prompt = self.generate_step_prompt(step, variables);

            // Execute step
            let execution_result = self
                .executor
                .execute_prompt(&context.workspace, &prompt)
                .await;

            let attempt_end = Utc::now();
            let duration_ms = (attempt_end - attempt_start).num_milliseconds() as u64;

            match execution_result {
                Ok(result) => {
                    return StepExecutionResult {
                        step_id: step.id.clone(),
                        success: result.success,
                        execution_result: Some(result),
                        error: None,
                        retry_count,
                        duration_ms,
                        started_at,
                        completed_at: attempt_end,
                    };
                }
                Err(e) => {
                    if retry_count >= context.config.max_retries {
                        return StepExecutionResult {
                            step_id: step.id.clone(),
                            success: false,
                            execution_result: None,
                            error: Some(e.to_string()),
                            retry_count,
                            duration_ms,
                            started_at,
                            completed_at: attempt_end,
                        };
                    }

                    retry_count += 1;

                    // Wait before retry
                    tokio::time::sleep(std::time::Duration::from_secs(
                        context.config.retry_delay_seconds,
                    ))
                    .await;
                }
            }
        }
    }

    /// Check if a step should be executed based on conditions
    fn should_execute_step(
        &self,
        step: &WorkflowStep,
        step_results: &HashMap<String, StepExecutionResult>,
        variables: &HashMap<String, String>,
    ) -> bool {
        if step.conditions.is_empty() {
            return true; // No conditions means always execute
        }

        for condition in &step.conditions {
            match &condition.condition_type {
                crate::prompt::ConditionType::PreviousStepSuccess => {
                    if let Some(previous_result) = step_results.get(&condition.value) {
                        if !previous_result.success {
                            return false;
                        }
                    } else {
                        return false; // Previous step hasn't executed
                    }
                }
                crate::prompt::ConditionType::PreviousStepFailure => {
                    if let Some(previous_result) = step_results.get(&condition.value) {
                        if previous_result.success {
                            return false;
                        }
                    } else {
                        return false; // Previous step hasn't executed
                    }
                }
                crate::prompt::ConditionType::VariableEquals { variable } => {
                    if let Some(value) = variables.get(variable) {
                        if value != &condition.value {
                            return false;
                        }
                    } else {
                        return false; // Variable doesn't exist
                    }
                }
                crate::prompt::ConditionType::CapabilityRequired => {
                    // This would need to be checked against agent capabilities
                    // For now, assume the condition is met
                    continue;
                }
                crate::prompt::ConditionType::Custom { expression: _ } => {
                    // Custom expressions would need a more complex evaluator
                    // For now, assume the condition is met
                    continue;
                }
            }
        }

        true
    }

    /// Generate a prompt for a workflow step
    fn generate_step_prompt(
        &self,
        step: &WorkflowStep,
        variables: &HashMap<String, String>,
    ) -> String {
        let mut prompt = format!("Execute workflow step: {}\n\n", step.name);
        prompt.push_str(&format!("Description: {}\n\n", step.description));

        // Add available variables
        if !variables.is_empty() {
            prompt.push_str("Available variables:\n");
            for (key, value) in variables {
                prompt.push_str(&format!("- {}: {}\n", key, value));
            }
            prompt.push('\n');
        }

        prompt
            .push_str("Please complete this step and provide a summary of what was accomplished.");

        prompt
    }

    /// Calculate workflow statistics from step results
    fn calculate_workflow_stats(
        &self,
        step_results: &HashMap<String, StepExecutionResult>,
        started_at: chrono::DateTime<chrono::Utc>,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> WorkflowStats {
        let total_duration_ms = (completed_at - started_at).num_milliseconds() as u64;
        let steps_executed = step_results.len() as u32;
        let total_retries = step_results.values().map(|r| r.retry_count).sum::<u32>();

        let total_cost = step_results
            .values()
            .filter_map(|r| {
                r.execution_result
                    .as_ref()
                    .and_then(|er| er.usage.as_ref())
                    .and_then(|u| u.cost_usd)
            })
            .sum::<f64>();

        WorkflowStats {
            total_duration_ms,
            steps_executed,
            total_retries,
            peak_memory_mb: None, // Would need system monitoring to calculate
            total_cost: if total_cost > 0.0 {
                Some(total_cost)
            } else {
                None
            },
        }
    }

    /// Get status of an active workflow
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Option<WorkflowExecutionContext> {
        self.active_workflows
            .read()
            .await
            .get(&workflow_id)
            .cloned()
    }

    /// Cancel an active workflow
    pub async fn cancel_workflow(&self, workflow_id: Uuid) -> Result<()> {
        if let Some(context) = self.active_workflows.write().await.get_mut(&workflow_id) {
            context.status = WorkflowStatus::Cancelled;
            Ok(())
        } else {
            Err(Error::NotFound {
                entity_type: "workflow".to_string(),
                id: workflow_id.to_string(),
            })
        }
    }

    /// List all active workflows
    pub async fn list_active_workflows(&self) -> Vec<WorkflowExecutionContext> {
        self.active_workflows
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::template_manager::FilesystemTemplateManager;
    use crate::prompt::{StepCondition, WorkflowStep};
    use chrono::Utc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workflow_executor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();

        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        assert_eq!(workflow_executor.active_workflows.read().await.len(), 0);
    }

    #[test]
    fn test_workflow_config_default() {
        let config = WorkflowConfig::default();

        assert_eq!(config.timeout_seconds, 1800);
        assert!(!config.continue_on_failure);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.retry_delay_seconds, 5);
        assert!(!config.enable_parallelism);
        assert_eq!(config.max_parallel_executions, 4);
    }

    #[tokio::test]
    async fn test_should_execute_step() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();
        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        // Step with no conditions should always execute
        let step_no_conditions = WorkflowStep {
            id: "step1".to_string(),
            name: "Step 1".to_string(),
            description: "First step".to_string(),
            order: 1,
            conditions: Vec::new(),
            timeout_seconds: None,
            retry_policy: None,
        };

        let step_results = HashMap::new();
        let variables = HashMap::new();

        assert!(workflow_executor.should_execute_step(
            &step_no_conditions,
            &step_results,
            &variables
        ));

        // Step with previous step success condition
        let step_with_condition = WorkflowStep {
            id: "step2".to_string(),
            name: "Step 2".to_string(),
            description: "Second step".to_string(),
            order: 2,
            conditions: vec![StepCondition {
                condition_type: crate::prompt::ConditionType::PreviousStepSuccess,
                value: "step1".to_string(),
            }],
            timeout_seconds: None,
            retry_policy: None,
        };

        // Should not execute if previous step doesn't exist
        assert!(!workflow_executor.should_execute_step(
            &step_with_condition,
            &step_results,
            &variables
        ));

        // Add successful previous step result
        let mut step_results_with_success = HashMap::new();
        step_results_with_success.insert(
            "step1".to_string(),
            StepExecutionResult {
                step_id: "step1".to_string(),
                success: true,
                execution_result: None,
                error: None,
                retry_count: 0,
                duration_ms: 1000,
                started_at: Utc::now(),
                completed_at: Utc::now(),
            },
        );

        // Should execute if previous step succeeded
        assert!(workflow_executor.should_execute_step(
            &step_with_condition,
            &step_results_with_success,
            &variables
        ));

        // Add failed previous step result
        let mut step_results_with_failure = HashMap::new();
        step_results_with_failure.insert(
            "step1".to_string(),
            StepExecutionResult {
                step_id: "step1".to_string(),
                success: false,
                execution_result: None,
                error: Some("Step failed".to_string()),
                retry_count: 1,
                duration_ms: 1000,
                started_at: Utc::now(),
                completed_at: Utc::now(),
            },
        );

        // Should not execute if previous step failed
        assert!(!workflow_executor.should_execute_step(
            &step_with_condition,
            &step_results_with_failure,
            &variables
        ));
    }

    #[tokio::test]
    async fn test_generate_step_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();
        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        let step = WorkflowStep {
            id: "test_step".to_string(),
            name: "Test Step".to_string(),
            description: "A test workflow step".to_string(),
            order: 1,
            conditions: Vec::new(),
            timeout_seconds: None,
            retry_policy: None,
        };

        let mut variables = HashMap::new();
        variables.insert("project_name".to_string(), "TestProject".to_string());
        variables.insert("version".to_string(), "1.0.0".to_string());

        let prompt = workflow_executor.generate_step_prompt(&step, &variables);

        assert!(prompt.contains("Test Step"));
        assert!(prompt.contains("A test workflow step"));
        assert!(prompt.contains("project_name: TestProject"));
        assert!(prompt.contains("version: 1.0.0"));
    }

    #[test]
    fn test_step_execution_result_creation() {
        let started_at = Utc::now();
        let completed_at = started_at + chrono::Duration::seconds(30);

        let result = StepExecutionResult {
            step_id: "test_step".to_string(),
            success: true,
            execution_result: None,
            error: None,
            retry_count: 0,
            duration_ms: 30000,
            started_at,
            completed_at,
        };

        assert_eq!(result.step_id, "test_step");
        assert!(result.success);
        assert_eq!(result.retry_count, 0);
        assert_eq!(result.duration_ms, 30000);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_workflow_status_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();
        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        let workflow_id = Uuid::new_v4();
        let context = WorkflowExecutionContext {
            workflow_id,
            current_step: "step1".to_string(),
            variables: HashMap::new(),
            step_results: HashMap::new(),
            metadata: HashMap::new(),
            started_at: Utc::now(),
            status: WorkflowStatus::Running,
        };

        // Add workflow to active list
        workflow_executor
            .active_workflows
            .write()
            .await
            .insert(workflow_id, context.clone());

        // Get status
        let status = workflow_executor.get_workflow_status(workflow_id).await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().workflow_id, workflow_id);

        // List active workflows
        let active = workflow_executor.list_active_workflows().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].workflow_id, workflow_id);

        // Cancel workflow
        workflow_executor
            .cancel_workflow(workflow_id)
            .await
            .unwrap();

        let updated_status = workflow_executor.get_workflow_status(workflow_id).await;
        assert!(updated_status.is_some());
        assert_eq!(updated_status.unwrap().status, WorkflowStatus::Cancelled);
    }

    #[test]
    fn test_calculate_workflow_stats() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();
        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        let started_at = Utc::now();
        let completed_at = started_at + chrono::Duration::minutes(5);

        let mut step_results = HashMap::new();

        // Add step result with retry
        step_results.insert(
            "step1".to_string(),
            StepExecutionResult {
                step_id: "step1".to_string(),
                success: true,
                execution_result: None,
                error: None,
                retry_count: 2,
                duration_ms: 120000,
                started_at,
                completed_at: started_at + chrono::Duration::minutes(2),
            },
        );

        // Add another step result
        step_results.insert(
            "step2".to_string(),
            StepExecutionResult {
                step_id: "step2".to_string(),
                success: true,
                execution_result: None,
                error: None,
                retry_count: 0,
                duration_ms: 180000,
                started_at: started_at + chrono::Duration::minutes(2),
                completed_at,
            },
        );

        let stats =
            workflow_executor.calculate_workflow_stats(&step_results, started_at, completed_at);

        assert_eq!(stats.steps_executed, 2);
        assert_eq!(stats.total_retries, 2);
        assert_eq!(stats.total_duration_ms, 300000); // 5 minutes
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let template_manager = Arc::new(FilesystemTemplateManager::new(temp_dir.path()));
        let executor = HeadlessClaudeExecutor::new();
        let workflow_executor = WorkflowExecutor::new(executor, template_manager);

        let nonexistent_id = Uuid::new_v4();
        let result = workflow_executor.cancel_workflow(nonexistent_id).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotFound { .. }));
    }
}
