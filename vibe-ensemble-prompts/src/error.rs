//! Error types for prompt management

use thiserror::Error;

/// Prompt management error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] vibe_ensemble_storage::Error),

    #[error("Core domain error: {0}")]
    Core(#[from] vibe_ensemble_core::Error),

    #[error("Template rendering error: {0}")]
    TemplateRendering(String),

    #[error("Template variable missing: {name}")]
    MissingVariable { name: String },

    #[error("Invalid template syntax: {0}")]
    InvalidTemplate(String),

    #[error("Prompt not found: {id}")]
    PromptNotFound { id: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for prompt operations
pub type Result<T> = std::result::Result<T, Error>;
