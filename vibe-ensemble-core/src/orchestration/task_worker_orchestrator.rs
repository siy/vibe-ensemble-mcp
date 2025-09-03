//! Task-Worker Integration Orchestrator
//!
//! This module provides the orchestration layer that bridges the task management system
//! with the worker management system, enabling automatic worker spawning for tasks.

use super::worker_manager::{McpServerConfig, WorkerManager, WorkerOutputConfig};
use crate::{
    issue::{Issue, IssuePriority},
    prompt::{PromptType, PromptVariable, RenderedPrompt, SystemPrompt, VariableType},
    Error, Result,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for task-worker orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkerConfig {
    /// Maximum number of concurrent workers per task type
    pub max_workers_per_type: usize,
    /// Default timeout for task completion in seconds
    pub default_task_timeout_seconds: u64,
    /// Maximum number of retry attempts for failed tasks
    pub max_retry_attempts: u32,
    /// Delay between retry attempts in seconds
    pub retry_delay_seconds: u64,
    /// Enable automatic worker cleanup on task completion
    pub auto_cleanup: bool,
}

impl Default for TaskWorkerConfig {
    fn default() -> Self {
        Self {
            max_workers_per_type: 5,
            default_task_timeout_seconds: 3600, // 1 hour
            max_retry_attempts: 3,
            retry_delay_seconds: 30,
            auto_cleanup: true,
        }
    }
}

/// Task-Worker mapping information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkerMapping {
    pub task_id: Uuid,
    pub worker_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub status: TaskWorkerStatus,
    pub retry_count: u32,
    pub last_retry_at: Option<DateTime<Utc>>,
    pub prompt_used: String,
    pub capabilities_required: Vec<String>,
}

/// Status of task-worker mapping
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskWorkerStatus {
    /// Worker is being spawned for the task
    Spawning,
    /// Worker is assigned and working on the task
    Active,
    /// Task completed successfully
    Completed,
    /// Task failed and may be retried
    Failed,
    /// Task failed permanently (max retries reached)
    FailedPermanently,
    /// Task was cancelled
    Cancelled,
}

/// Task-specific prompt generator for creating worker prompts based on task metadata
#[derive(Debug, Clone)]
pub struct TaskPromptGenerator {
    /// Base prompt templates for different task types
    base_templates: HashMap<String, SystemPrompt>,
}

impl TaskPromptGenerator {
    /// Create a new task prompt generator with default templates
    pub fn new() -> Result<Self> {
        let mut base_templates = HashMap::new();

        // General task template
        let general_template = SystemPrompt::builder()
            .name("general-task-worker")
            .description("General purpose task worker prompt")
            .template(Self::general_task_template())
            .prompt_type(PromptType::Worker)
            .created_by(Uuid::new_v4())
            .variable(PromptVariable::new(
                "task_title".to_string(),
                "Title of the task to complete".to_string(),
                VariableType::String,
                true,
            )?)
            .variable(PromptVariable::new(
                "task_description".to_string(),
                "Detailed description of the task".to_string(),
                VariableType::String,
                true,
            )?)
            .variable(PromptVariable::new(
                "task_priority".to_string(),
                "Priority level of the task".to_string(),
                VariableType::String,
                true,
            )?)
            .variable(
                PromptVariable::new(
                    "task_tags".to_string(),
                    "Comma-separated tags for the task".to_string(),
                    VariableType::String,
                    false,
                )?
                .with_default_value("general".to_string()),
            )
            .variable(
                PromptVariable::new(
                    "task_capabilities".to_string(),
                    "Required capabilities for this task".to_string(),
                    VariableType::String,
                    false,
                )?
                .with_default_value("general".to_string()),
            )
            .build()?;

        base_templates.insert("general".to_string(), general_template);

        // Code review task template
        let code_review_template = SystemPrompt::builder()
            .name("code-review-task-worker")
            .description("Code review task worker prompt")
            .template(Self::code_review_task_template())
            .prompt_type(PromptType::Worker)
            .created_by(Uuid::new_v4())
            .variable(PromptVariable::new(
                "task_title".to_string(),
                "Title of the code review task".to_string(),
                VariableType::String,
                true,
            )?)
            .variable(PromptVariable::new(
                "task_description".to_string(),
                "Description of what needs to be reviewed".to_string(),
                VariableType::String,
                true,
            )?)
            .variable(PromptVariable::new(
                "task_priority".to_string(),
                "Priority level of the review".to_string(),
                VariableType::String,
                true,
            )?)
            .build()?;

        base_templates.insert("code-review".to_string(), code_review_template);

        Ok(Self { base_templates })
    }

    /// Generate a task-specific prompt based on issue metadata
    pub fn generate_prompt(&self, issue: &Issue) -> Result<RenderedPrompt> {
        // Determine task type from tags
        let task_type = self.determine_task_type(issue);

        // Get appropriate template
        let template = self
            .base_templates
            .get(&task_type)
            .unwrap_or(self.base_templates.get("general").unwrap());

        // Prepare variables
        let mut variables = HashMap::new();
        variables.insert("task_title".to_string(), issue.title.clone());
        variables.insert("task_description".to_string(), issue.description.clone());
        variables.insert("task_priority".to_string(), issue.priority.to_string());
        variables.insert("task_tags".to_string(), issue.tags.join(", "));

        // Determine required capabilities from tags
        let capabilities = self.determine_capabilities(issue);
        variables.insert("task_capabilities".to_string(), capabilities.join(", "));

        template.render(&variables)
    }

    /// Determine task type from issue tags and description
    fn determine_task_type(&self, issue: &Issue) -> String {
        // Check for specific task type tags
        for tag in &issue.tags {
            match tag.as_str() {
                "code-review" | "review" => return "code-review".to_string(),
                "bug" | "fix" => return "bug-fix".to_string(),
                "feature" | "enhancement" => return "feature".to_string(),
                "test" | "testing" => return "testing".to_string(),
                "documentation" | "docs" => return "documentation".to_string(),
                _ => {}
            }
        }

        // Check description for keywords
        let description_lower = issue.description.to_lowercase();
        if description_lower.contains("review") || description_lower.contains("code review") {
            return "code-review".to_string();
        }
        if description_lower.contains("bug") || description_lower.contains("fix") {
            return "bug-fix".to_string();
        }
        if description_lower.contains("test") || description_lower.contains("testing") {
            return "testing".to_string();
        }

        "general".to_string()
    }

    /// Determine required capabilities from issue metadata
    fn determine_capabilities(&self, issue: &Issue) -> Vec<String> {
        let mut capabilities = Vec::new();

        // Add capabilities based on tags
        for tag in &issue.tags {
            match tag.as_str() {
                "rust" => capabilities.push("rust-development".to_string()),
                "javascript" | "js" => capabilities.push("javascript-development".to_string()),
                "python" => capabilities.push("python-development".to_string()),
                "code-review" => capabilities.push("code-review".to_string()),
                "testing" => capabilities.push("testing".to_string()),
                "documentation" => capabilities.push("documentation".to_string()),
                "security" => capabilities.push("security-analysis".to_string()),
                "performance" => capabilities.push("performance-analysis".to_string()),
                _ => {}
            }
        }

        // Add default capability if none found
        if capabilities.is_empty() {
            capabilities.push("general".to_string());
        }

        // Add priority-based capabilities
        match issue.priority {
            IssuePriority::Critical | IssuePriority::High => {
                capabilities.push("high-priority-handling".to_string());
            }
            _ => {}
        }

        capabilities
    }

    /// General task template
    fn general_task_template() -> String {
        r#"You are a skilled Claude Code worker agent assigned to complete a specific task.

# Task Information
- **Title**: {{task_title}}
- **Description**: {{task_description}}
- **Priority**: {{task_priority}}
- **Tags**: {{task_tags}}
- **Required Capabilities**: {{task_capabilities}}

# Your Role
As a task worker, your primary responsibilities are:
1. **Understand the Task**: Carefully analyze the task title, description, and requirements
2. **Plan Your Approach**: Break down the task into manageable steps
3. **Execute Systematically**: Complete the task following best practices
4. **Communicate Progress**: Provide clear updates on your progress
5. **Quality Assurance**: Ensure your work meets high standards before completion

# Task Execution Guidelines
- Follow the task description precisely
- If the task is unclear, ask for clarification before proceeding
- Use appropriate tools and techniques for the task type
- Test your work thoroughly before marking complete
- Document your approach and any important decisions
- Handle errors gracefully and report issues promptly

# Priority Guidelines
{{#if (eq task_priority "Critical")}}
⚠️ **CRITICAL PRIORITY**: This task requires immediate attention and exceptional care. Prioritize accuracy and completeness.
{{else if (eq task_priority "High")}}
🔴 **HIGH PRIORITY**: Complete this task promptly while maintaining quality standards.
{{else if (eq task_priority "Medium")}}
🟡 **MEDIUM PRIORITY**: Standard task completion timeline and quality expectations apply.
{{else}}
🟢 **LOW PRIORITY**: Take time to ensure thorough completion without rushing.
{{/if}}

# Success Criteria
- Task objectives are fully met
- Work quality meets or exceeds expectations
- Any deliverables are properly documented
- No breaking changes or regressions introduced
- All testing requirements satisfied

Begin by acknowledging this task assignment and outlining your planned approach."#.to_string()
    }

    /// Code review task template
    fn code_review_task_template() -> String {
        r#"You are a specialized Claude Code worker agent focused on code review tasks.

# Code Review Task
- **Title**: {{task_title}}
- **Description**: {{task_description}}
- **Priority**: {{task_priority}}

# Your Code Review Responsibilities
1. **Code Quality Analysis**: Review code for style, structure, and maintainability
2. **Logic Verification**: Ensure the code logic is correct and efficient
3. **Security Assessment**: Identify potential security vulnerabilities
4. **Performance Review**: Check for performance issues and optimization opportunities
5. **Best Practices**: Verify adherence to coding standards and best practices
6. **Testing Coverage**: Assess test coverage and suggest improvements
7. **Documentation Review**: Ensure code is properly documented

# Review Process
1. **Initial Scan**: Get an overview of the changes/code to review
2. **Detailed Analysis**: Go through the code systematically
3. **Issue Identification**: Note any problems, suggestions, or questions
4. **Priority Assessment**: Categorize findings by severity
5. **Recommendation Formulation**: Provide clear, actionable feedback
6. **Summary Creation**: Compile findings into a comprehensive review report

# Review Categories
- **Critical**: Issues that must be fixed (security, correctness)
- **Important**: Significant improvements needed (performance, maintainability)
- **Minor**: Style or preference improvements
- **Suggestion**: Optional enhancements or alternative approaches

# Output Format
Provide your review as a structured report with:
- Executive summary
- Critical issues (if any)
- Important findings
- Minor issues and suggestions
- Overall assessment and recommendation

{{#if (eq task_priority "Critical")}}
⚠️ **CRITICAL REVIEW**: This code review is high-priority. Be extra thorough in your analysis.
{{/if}}

Begin by confirming the scope of your review and your planned approach."#
            .to_string()
    }
}

impl Default for TaskPromptGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to create default TaskPromptGenerator")
    }
}

/// Main orchestrator for task-worker integration
pub struct TaskWorkerOrchestrator {
    /// Worker manager for spawning and managing workers
    worker_manager: Arc<WorkerManager>,
    /// Task-worker mappings
    mappings: Arc<RwLock<HashMap<Uuid, TaskWorkerMapping>>>,
    /// Reverse mapping: worker_id -> task_id
    worker_to_task: Arc<RwLock<HashMap<Uuid, Uuid>>>,
    /// Prompt generator for creating task-specific prompts
    prompt_generator: TaskPromptGenerator,
    /// Configuration
    config: TaskWorkerConfig,
}

impl TaskWorkerOrchestrator {
    /// Create a new task-worker orchestrator
    pub fn new(
        mcp_config: McpServerConfig,
        output_config: WorkerOutputConfig,
        config: TaskWorkerConfig,
    ) -> Result<Self> {
        let worker_manager = Arc::new(WorkerManager::new(mcp_config, output_config));
        let prompt_generator = TaskPromptGenerator::new()?;

        Ok(Self {
            worker_manager,
            mappings: Arc::new(RwLock::new(HashMap::new())),
            worker_to_task: Arc::new(RwLock::new(HashMap::new())),
            prompt_generator,
            config,
        })
    }

    /// Assign a task to a worker (spawn worker automatically)
    pub async fn assign_task_to_worker(
        &self,
        mut issue: Issue,
        working_directory: Option<PathBuf>,
    ) -> Result<Uuid> {
        // Ensure task can be assigned
        if !issue.can_be_assigned() {
            return Err(Error::state_transition(format!(
                "Task {} cannot be assigned in status: {}",
                issue.id, issue.status
            )));
        }

        // Generate task-specific prompt
        let rendered_prompt = self.prompt_generator.generate_prompt(&issue)?;

        // Determine required capabilities
        let capabilities = self.prompt_generator.determine_capabilities(&issue);

        // Spawn worker with task-specific prompt
        let worker_id = self
            .worker_manager
            .spawn_worker(
                rendered_prompt.content.clone(),
                capabilities.clone(),
                working_directory,
            )
            .await?;

        // Update issue status and assignment
        issue.assign_to(worker_id);

        // Create task-worker mapping
        let mapping = TaskWorkerMapping {
            task_id: issue.id,
            worker_id,
            assigned_at: Utc::now(),
            status: TaskWorkerStatus::Spawning,
            retry_count: 0,
            last_retry_at: None,
            prompt_used: rendered_prompt.content,
            capabilities_required: capabilities,
        };

        // Store mappings
        self.mappings.write().await.insert(issue.id, mapping);
        self.worker_to_task
            .write()
            .await
            .insert(worker_id, issue.id);

        Ok(worker_id)
    }

    /// Handle task completion (successful)
    pub async fn complete_task(&self, task_id: Uuid) -> Result<()> {
        let mut mappings = self.mappings.write().await;

        if let Some(mapping) = mappings.get_mut(&task_id) {
            mapping.status = TaskWorkerStatus::Completed;

            // Auto-cleanup worker if configured
            if self.config.auto_cleanup {
                if let Err(e) = self
                    .worker_manager
                    .shutdown_worker(&mapping.worker_id)
                    .await
                {
                    tracing::warn!(
                        "Failed to cleanup worker {} for completed task {}: {}",
                        mapping.worker_id,
                        task_id,
                        e
                    );
                }
            }

            // Clean up reverse mapping
            self.worker_to_task.write().await.remove(&mapping.worker_id);

            Ok(())
        } else {
            Err(Error::NotFound {
                entity_type: "TaskWorkerMapping".to_string(),
                id: task_id.to_string(),
            })
        }
    }

    /// Handle task failure
    pub async fn fail_task(&self, task_id: Uuid, error_message: String) -> Result<()> {
        let mut mappings = self.mappings.write().await;

        if let Some(mapping) = mappings.get_mut(&task_id) {
            mapping.retry_count += 1;
            mapping.last_retry_at = Some(Utc::now());

            if mapping.retry_count >= self.config.max_retry_attempts {
                // Mark as permanently failed
                mapping.status = TaskWorkerStatus::FailedPermanently;

                // Cleanup worker
                if let Err(e) = self
                    .worker_manager
                    .shutdown_worker(&mapping.worker_id)
                    .await
                {
                    tracing::warn!(
                        "Failed to cleanup worker {} for failed task {}: {}",
                        mapping.worker_id,
                        task_id,
                        e
                    );
                }

                self.worker_to_task.write().await.remove(&mapping.worker_id);
            } else {
                // Mark for retry
                mapping.status = TaskWorkerStatus::Failed;
            }

            tracing::info!(
                "Task {} failed (attempt {} of {}): {}",
                task_id,
                mapping.retry_count,
                self.config.max_retry_attempts,
                error_message
            );

            Ok(())
        } else {
            Err(Error::NotFound {
                entity_type: "TaskWorkerMapping".to_string(),
                id: task_id.to_string(),
            })
        }
    }

    /// Retry a failed task
    pub async fn retry_task(
        &self,
        task_id: Uuid,
        working_directory: Option<PathBuf>,
    ) -> Result<Uuid> {
        let mappings = self.mappings.read().await;

        if let Some(mapping) = mappings.get(&task_id) {
            if mapping.status != TaskWorkerStatus::Failed {
                return Err(Error::state_transition(format!(
                    "Task {} cannot be retried in status: {:?}",
                    task_id, mapping.status
                )));
            }

            // Shutdown old worker if still running
            let _ = self
                .worker_manager
                .shutdown_worker(&mapping.worker_id)
                .await;

            // Spawn new worker with same prompt and capabilities
            let new_worker_id = self
                .worker_manager
                .spawn_worker(
                    mapping.prompt_used.clone(),
                    mapping.capabilities_required.clone(),
                    working_directory,
                )
                .await?;

            drop(mappings);

            // Update mapping
            let mut mappings = self.mappings.write().await;
            if let Some(mapping) = mappings.get_mut(&task_id) {
                let old_worker_id = mapping.worker_id;
                mapping.worker_id = new_worker_id;
                mapping.status = TaskWorkerStatus::Spawning;

                // Update reverse mapping
                let mut worker_to_task = self.worker_to_task.write().await;
                worker_to_task.remove(&old_worker_id);
                worker_to_task.insert(new_worker_id, task_id);
            }

            Ok(new_worker_id)
        } else {
            Err(Error::NotFound {
                entity_type: "TaskWorkerMapping".to_string(),
                id: task_id.to_string(),
            })
        }
    }

    /// Cancel a task and cleanup its worker
    pub async fn cancel_task(&self, task_id: Uuid) -> Result<()> {
        let mut mappings = self.mappings.write().await;

        if let Some(mapping) = mappings.get_mut(&task_id) {
            mapping.status = TaskWorkerStatus::Cancelled;

            // Shutdown worker
            if let Err(e) = self
                .worker_manager
                .shutdown_worker(&mapping.worker_id)
                .await
            {
                tracing::warn!(
                    "Failed to shutdown worker {} for cancelled task {}: {}",
                    mapping.worker_id,
                    task_id,
                    e
                );
            }

            // Clean up reverse mapping
            self.worker_to_task.write().await.remove(&mapping.worker_id);

            Ok(())
        } else {
            Err(Error::NotFound {
                entity_type: "TaskWorkerMapping".to_string(),
                id: task_id.to_string(),
            })
        }
    }

    /// Get task-worker mapping by task ID
    pub async fn get_task_mapping(&self, task_id: Uuid) -> Option<TaskWorkerMapping> {
        self.mappings.read().await.get(&task_id).cloned()
    }

    /// Get task ID by worker ID
    pub async fn get_task_by_worker(&self, worker_id: Uuid) -> Option<Uuid> {
        self.worker_to_task.read().await.get(&worker_id).copied()
    }

    /// List all active task-worker mappings
    pub async fn list_active_mappings(&self) -> Vec<TaskWorkerMapping> {
        self.mappings
            .read()
            .await
            .values()
            .filter(|mapping| {
                matches!(
                    mapping.status,
                    TaskWorkerStatus::Spawning | TaskWorkerStatus::Active
                )
            })
            .cloned()
            .collect()
    }

    /// Handle worker disconnection
    pub async fn handle_worker_disconnection(&self, worker_id: Uuid) -> Result<()> {
        if let Some(task_id) = self.get_task_by_worker(worker_id).await {
            tracing::warn!(
                "Worker {} disconnected while working on task {}",
                worker_id,
                task_id
            );

            // Mark task as failed due to worker disconnection
            self.fail_task(task_id, "Worker disconnected unexpectedly".to_string())
                .await?;
        }

        // Delegate to worker manager
        self.worker_manager
            .handle_worker_disconnection(&worker_id.to_string())
            .await
    }

    /// Shutdown all workers and cleanup
    pub async fn shutdown_all(&self) -> Result<()> {
        tracing::info!("Shutting down all task workers");

        // Mark all active tasks as cancelled
        let mut mappings = self.mappings.write().await;
        for mapping in mappings.values_mut() {
            if matches!(
                mapping.status,
                TaskWorkerStatus::Spawning | TaskWorkerStatus::Active
            ) {
                mapping.status = TaskWorkerStatus::Cancelled;
            }
        }
        drop(mappings);

        // Clear reverse mappings
        self.worker_to_task.write().await.clear();

        // Shutdown all workers
        self.worker_manager.shutdown_all().await
    }

    /// Get orchestrator statistics
    pub async fn get_stats(&self) -> TaskWorkerStats {
        let mappings = self.mappings.read().await;

        let mut stats = TaskWorkerStats {
            total_tasks: mappings.len(),
            active_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            cancelled_tasks: 0,
            average_retry_count: 0.0,
        };

        let mut total_retries = 0;

        for mapping in mappings.values() {
            total_retries += mapping.retry_count;

            match mapping.status {
                TaskWorkerStatus::Spawning | TaskWorkerStatus::Active => stats.active_tasks += 1,
                TaskWorkerStatus::Completed => stats.completed_tasks += 1,
                TaskWorkerStatus::Failed | TaskWorkerStatus::FailedPermanently => {
                    stats.failed_tasks += 1
                }
                TaskWorkerStatus::Cancelled => stats.cancelled_tasks += 1,
            }
        }

        if !mappings.is_empty() {
            stats.average_retry_count = total_retries as f64 / mappings.len() as f64;
        }

        stats
    }
}

/// Statistics about task-worker orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWorkerStats {
    pub total_tasks: usize,
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub cancelled_tasks: usize,
    pub average_retry_count: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::{Issue, IssuePriority};

    #[tokio::test]
    async fn test_task_prompt_generator_general() {
        let generator = TaskPromptGenerator::new().unwrap();

        let issue = Issue::builder()
            .title("Fix authentication bug")
            .description("The login system is not properly validating user credentials")
            .priority(IssuePriority::High)
            .tag("bug")
            .tag("security")
            .build()
            .unwrap();

        let prompt = generator.generate_prompt(&issue).unwrap();

        assert!(prompt.content.contains("Fix authentication bug"));
        assert!(prompt.content.contains("login system"));
        assert!(prompt.content.contains("High"));
        assert!(prompt.content.contains("bug, security"));
    }

    #[tokio::test]
    async fn test_task_prompt_generator_code_review() {
        let generator = TaskPromptGenerator::new().unwrap();

        let issue = Issue::builder()
            .title("Review pull request #123")
            .description("Please review the new authentication module implementation")
            .priority(IssuePriority::Medium)
            .tag("code-review")
            .tag("security")
            .build()
            .unwrap();

        let prompt = generator.generate_prompt(&issue).unwrap();

        assert!(prompt.content.contains("Code Review Task"));
        assert!(prompt.content.contains("Review pull request #123"));
        assert!(prompt.content.contains("authentication module"));
    }

    #[test]
    fn test_capability_determination() {
        let generator = TaskPromptGenerator::new().unwrap();

        let issue = Issue::builder()
            .title("Fix Rust compilation error")
            .description("The code doesn't compile due to lifetime issues")
            .priority(IssuePriority::Critical)
            .tag("rust")
            .tag("bug")
            .build()
            .unwrap();

        let capabilities = generator.determine_capabilities(&issue);

        assert!(capabilities.contains(&"rust-development".to_string()));
        assert!(capabilities.contains(&"high-priority-handling".to_string()));
    }

    #[test]
    fn test_task_type_determination() {
        let generator = TaskPromptGenerator::new().unwrap();

        // Test code review detection
        let review_issue = Issue::builder()
            .title("Review PR")
            .description("Please review this code")
            .priority(IssuePriority::Medium)
            .tag("code-review")
            .build()
            .unwrap();

        assert_eq!(generator.determine_task_type(&review_issue), "code-review");

        // Test bug fix detection
        let bug_issue = Issue::builder()
            .title("Fix bug")
            .description("There's a bug in the system")
            .priority(IssuePriority::High)
            .tag("bug")
            .build()
            .unwrap();

        assert_eq!(generator.determine_task_type(&bug_issue), "bug-fix");

        // Test general task fallback
        let general_issue = Issue::builder()
            .title("Some task")
            .description("Do something")
            .priority(IssuePriority::Low)
            .build()
            .unwrap();

        assert_eq!(generator.determine_task_type(&general_issue), "general");
    }

    #[test]
    fn test_task_worker_config_default() {
        let config = TaskWorkerConfig::default();

        assert_eq!(config.max_workers_per_type, 5);
        assert_eq!(config.default_task_timeout_seconds, 3600);
        assert_eq!(config.max_retry_attempts, 3);
        assert_eq!(config.retry_delay_seconds, 30);
        assert!(config.auto_cleanup);
    }

    #[test]
    fn test_task_worker_mapping_creation() {
        let task_id = Uuid::new_v4();
        let worker_id = Uuid::new_v4();
        let now = Utc::now();

        let mapping = TaskWorkerMapping {
            task_id,
            worker_id,
            assigned_at: now,
            status: TaskWorkerStatus::Spawning,
            retry_count: 0,
            last_retry_at: None,
            prompt_used: "Test prompt".to_string(),
            capabilities_required: vec!["general".to_string()],
        };

        assert_eq!(mapping.task_id, task_id);
        assert_eq!(mapping.worker_id, worker_id);
        assert_eq!(mapping.status, TaskWorkerStatus::Spawning);
        assert_eq!(mapping.retry_count, 0);
        assert!(mapping.last_retry_at.is_none());
    }
}
