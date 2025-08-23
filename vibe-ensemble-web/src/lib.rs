//! Web Dashboard for Vibe Ensemble Coordination Management
//!
//! This crate provides a web-based dashboard for visualizing and managing
//! the Vibe Ensemble coordination system, including agents, issues, and
//! coordination activities.

pub mod error;
pub mod handlers;
pub mod server;
pub mod templates;

pub use error::{Error, Result};
pub use server::WebServer;

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
pub use vibe_ensemble_storage as storage;
