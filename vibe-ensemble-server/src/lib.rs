//! Main server application for Vibe Ensemble MCP
//!
//! This crate provides the main server application that orchestrates
//! all components of the Vibe Ensemble system.

pub mod config;
pub mod error;
pub mod server;

pub use error::{Error, Result};

/// Re-export all core modules for convenience
pub use vibe_ensemble_core as core;
pub use vibe_ensemble_mcp as mcp;
pub use vibe_ensemble_storage as storage;
pub use vibe_ensemble_web as web;
pub use vibe_ensemble_prompts as prompts;