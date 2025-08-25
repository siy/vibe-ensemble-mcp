//! Error types for the server application

use thiserror::Error;

/// Server application error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Config file error: {0}")]
    ConfigFile(#[from] config::ConfigError),

    #[error("MCP protocol error: {0}")]
    Mcp(#[from] vibe_ensemble_mcp::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] vibe_ensemble_storage::Error),

    #[error("Web server error: {0}")]
    Web(#[from] vibe_ensemble_web::Error),

    #[error("Core domain error: {0}")]
    Core(#[from] vibe_ensemble_core::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for server operations
pub type Result<T> = std::result::Result<T, Error>;
