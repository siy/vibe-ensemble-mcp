//! Error types for MCP protocol operations

use thiserror::Error;

/// MCP protocol error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Protocol error: {message}")]
    Protocol { message: String },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Core domain error: {0}")]
    Core(#[from] vibe_ensemble_core::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for MCP operations
pub type Result<T> = std::result::Result<T, Error>;