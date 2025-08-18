//! Error types for storage operations

use thiserror::Error;

/// Storage layer error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Core domain error: {0}")]
    Core(#[from] vibe_ensemble_core::Error),

    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for storage operations
pub type Result<T> = std::result::Result<T, Error>;
