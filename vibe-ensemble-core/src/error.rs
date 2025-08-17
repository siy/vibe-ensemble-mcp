//! Error types for the core domain

use thiserror::Error;

/// Core error type for domain operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("UUID parsing error: {0}")]
    UuidParse(#[from] uuid::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for core operations
pub type Result<T> = std::result::Result<T, Error>;