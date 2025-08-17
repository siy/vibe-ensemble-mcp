//! MCP protocol implementation for Vibe Ensemble
//!
//! This crate provides the Model Context Protocol (MCP) implementation
//! for communication between the Vibe Ensemble server and Claude Code instances.

pub mod client;
pub mod error;
pub mod protocol;
pub mod server;
pub mod transport;

pub use error::{Error, Result};

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;