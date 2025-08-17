//! System prompts management for Vibe Ensemble MCP server
//!
//! This crate provides functionality for managing system prompts,
//! rendering templates, and configuring AI agent behavior.

pub mod error;
pub mod manager;
pub mod renderer;
pub mod templates;

pub use error::{Error, Result};
pub use manager::PromptManager;
pub use renderer::PromptRenderer;

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
pub use vibe_ensemble_storage as storage;