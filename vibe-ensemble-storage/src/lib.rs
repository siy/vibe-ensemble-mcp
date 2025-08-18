//! Persistence layer for Vibe Ensemble MCP server
//!
//! This crate provides database storage and repository implementations
//! for all domain entities in the Vibe Ensemble system.

pub mod error;
pub mod manager;
pub mod migrations;
pub mod repositories;
pub mod services;

pub use error::{Error, Result};
pub use manager::StorageManager;
pub use services::{AgentService, AgentSession, AgentStatistics};

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
