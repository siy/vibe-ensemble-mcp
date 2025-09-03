//! Advanced Claude Code agent orchestration system
//!
//! This module provides comprehensive agent orchestration capabilities including:
//! - Filesystem-based agent template management
//! - Persistent workspace lifecycle management
//! - Headless Claude Code execution with structured JSON output
//! - Template variable substitution with Handlebars
//! - Workflow coordination and task distribution
//!
//! # Overview
//!
//! The orchestration system enables dynamic creation and management of Claude Code agents
//! through a template-based approach. Agent configurations are stored as filesystem
//! templates that can be instantiated into working environments with customized
//! parameters and capabilities.
//!
//! # Architecture
//!
//! ```text
//! Agent Templates (Filesystem)
//!     |
//!     v
//! Template Manager
//!     |
//!     v
//! Workspace Manager (Persistent)
//!     |
//!     v
//! Agent Configuration Generator
//!     |
//!     v
//! Headless Claude Executor
//!     |
//!     v
//! Workflow Coordinator
//! ```
//!
//! # Examples
//!
//! ## Basic usage example:
//!
//! ```rust,no_run
//! use vibe_ensemble_core::orchestration::{
//!     template_manager::{FilesystemTemplateManager, TemplateManager},
//!     workspace_manager::{WorkspaceManager, WorkspaceConfig},
//!     executor::HeadlessClaudeExecutor,
//! };
//! use std::path::Path;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let template_manager = FilesystemTemplateManager::new(Path::new("./agent-templates"));
//!     let workspace_manager = WorkspaceManager::new(Path::new("./workspaces"));
//!     let executor = HeadlessClaudeExecutor::new();
//!
//!     // Load template
//!     let template = template_manager.load_template("code-reviewer").await?;
//!     
//!     // Create workspace configuration
//!     let config = WorkspaceConfig::default();
//!
//!     // Create workspace
//!     let workspace = workspace_manager
//!         .create_workspace("review-session-1", &template, &config)
//!         .await?;
//!
//!     // Execute task
//!     let result = executor
//!         .execute_prompt(&workspace, "Review the code in src/main.rs")
//!         .await?;
//!
//!     println!("Review completed: {}", result.content);
//!     Ok(())
//! }
//! ```

pub mod executor;
pub mod models;
pub mod task_worker_orchestrator;
pub mod template_manager;
pub mod worker_manager;
pub mod workflow;
pub mod workspace_manager;

pub use executor::{ClaudeStreamEvent, ExecutionResult, HeadlessClaudeExecutor};
pub use models::*;
pub use task_worker_orchestrator::{
    TaskPromptGenerator, TaskWorkerConfig, TaskWorkerMapping, TaskWorkerOrchestrator,
    TaskWorkerStats, TaskWorkerStatus,
};
pub use template_manager::{FilesystemTemplateManager, TemplateManager};
pub use worker_manager::{
    McpServerConfig, OutputLine, OutputType, WorkerHandle, WorkerInfo, WorkerManager, WorkerOutput,
    WorkerOutputConfig, WorkerStatus,
};
pub use workflow::{WorkflowContext, WorkflowExecutor, WorkflowResult};
pub use workspace_manager::{WorkspaceConfig, WorkspaceManager};
