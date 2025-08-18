//! Web interface for Vibe Ensemble MCP server
//!
//! This crate provides a web-based interface for managing issues, agents,
//! and other aspects of the Vibe Ensemble system.

pub mod error;
pub mod handlers;
pub mod server;
pub mod templates;

pub use error::{Error, Result};
pub use server::WebServer;

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
pub use vibe_ensemble_storage as storage;
