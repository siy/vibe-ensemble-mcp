//! Core domain models and traits for Vibe Ensemble MCP server
//!
//! This crate contains the fundamental domain models, traits, and types
//! used throughout the Vibe Ensemble system for coordinating multiple
//! Claude Code instances.

pub mod agent;
pub mod error;
pub mod issue;
pub mod knowledge;
pub mod message;
pub mod prompt;

pub use error::{Error, Result};

/// Common result type used throughout the core library
pub type CoreResult<T> = std::result::Result<T, Error>;