//! Core domain models and traits for Vibe Ensemble MCP server
//!
//! This crate contains the fundamental domain models, traits, and types
//! used throughout the Vibe Ensemble system for coordinating multiple
//! Claude Code instances.
//!
//! # Overview
//!
//! The Vibe Ensemble system is designed to coordinate multiple Claude Code agents
//! working together on complex tasks. This core library provides the essential
//! domain models that represent the key entities in the system:
//!
//! - **[Agent]**: Represents a Claude Code agent with capabilities and connection metadata
//! - **[Issue]**: Tracks work items and tasks that need to be completed
//! - **[Message]**: Enables communication between agents with typed content and metadata
//! - **[Knowledge]**: Stores patterns, practices, and guidelines with versioning and access control
//! - **[Configuration]**: Manages coordinator settings and behavioral parameters
//! - **[SystemPrompt]**: Versioned prompts for different agent roles and capabilities
//! - **[AgentTemplate]**: Templates for configuring Claude Code agents with workflows
//!
//! # Error Handling
//!
//! All domain operations use the [`Error`] type which provides detailed error information
//! with categories for validation, state transitions, constraints, and more. The [`Result`]
//! type alias provides convenient error handling throughout the system.
//!
//! # Getting Started
//!
//! Here's a quick example of creating an agent and assigning it to an issue:
//!
//! ```rust
//! use vibe_ensemble_core::{agent::*, issue::*, Error, Result};
//! use uuid::Uuid;
//!
//! // Create connection metadata for the agent
//! let metadata = ConnectionMetadata::builder()
//!     .endpoint("https://localhost:8080")
//!     .protocol_version("1.0")
//!     .build()?;
//!
//! // Create a worker agent with capabilities
//! let agent = Agent::builder()
//!     .name("code-reviewer-01")
//!     .agent_type(AgentType::Worker)
//!     .capability("code-review")
//!     .capability("static-analysis")
//!     .connection_metadata(metadata)
//!     .build()?;
//!
//! // Create an issue that needs work
//! let mut issue = Issue::builder()
//!     .title("Review pull request #123")
//!     .description("Perform code review on the new authentication module")
//!     .priority(IssuePriority::High)
//!     .tag("code-review")
//!     .tag("security")
//!     .build()?;
//!
//! // Assign the issue to the agent
//! issue.assign_to(agent.id);
//!
//! println!("Agent {} assigned to issue: {}", agent.name, issue.title);
//! # Ok::<(), Error>(())
//! ```
//!
//! # Features
//!
//! - **Comprehensive Validation**: All models include thorough input validation with detailed error messages and constraint checking
//! - **Builder Patterns**: Fluent builder APIs for easy and safe model construction with validation at build time
//! - **Serialization**: All models support JSON serialization via serde with proper field naming
//! - **Versioning**: Models like Knowledge and SystemPrompt support versioning for change tracking and history
//! - **State Management**: Issue and Agent models include proper state transition validation with business logic enforcement
//! - **Access Control**: Knowledge models support role-based access control with public, team, and private access levels
//! - **Relationships**: Models can reference each other through UUIDs and maintain typed relationships
//! - **Error Handling**: Rich error types with categorization, recoverability flags, and detailed context
//! - **Test Coverage**: Comprehensive unit tests covering all validation scenarios, edge cases, and operations
//! - **Documentation**: Complete API documentation with usage examples and best practices
//!
//! [Agent]: agent::Agent
//! [Issue]: issue::Issue  
//! [Message]: message::Message
//! [Knowledge]: knowledge::Knowledge
//! [Configuration]: config::Configuration
//! [SystemPrompt]: prompt::SystemPrompt
//! [AgentTemplate]: prompt::AgentTemplate

pub mod agent;
pub mod config;
pub mod error;
pub mod issue;
pub mod knowledge;
pub mod knowledge_intelligence;
pub mod message;
pub mod orchestration;
pub mod prompt;

pub use error::{Error, Result};

/// Common result type used throughout the core library
pub type CoreResult<T> = std::result::Result<T, Error>;
