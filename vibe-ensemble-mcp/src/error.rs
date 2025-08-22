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

    #[error("Storage error: {0}")]
    Storage(#[from] vibe_ensemble_storage::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Invalid method: {method}")]
    InvalidMethod { method: String },

    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },
}

impl Error {
    /// Create a service unavailable error
    pub fn service_unavailable(message: &str) -> Self {
        Error::ServiceUnavailable {
            message: message.to_string(),
        }
    }

    /// Create a validation error
    pub fn validation(message: &str) -> Self {
        Error::InvalidParams {
            message: message.to_string(),
        }
    }
}

/// Convenience result type for MCP operations
pub type Result<T> = std::result::Result<T, Error>;
