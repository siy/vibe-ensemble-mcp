//! Error types for web interface

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Web interface error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] vibe_ensemble_storage::error::Error),

    #[error("Core domain error: {0}")]
    Core(#[from] vibe_ensemble_core::error::Error),

    #[error("Template error: {0}")]
    Template(#[from] askama::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience result type for web operations
pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Error::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Error::Storage(ref err) => {
                tracing::error!("Storage error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Error::Core(ref err) => {
                tracing::error!("Core error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Error::Template(ref err) => {
                tracing::error!("Template error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering error".to_string(),
                )
            }
            Error::Serialization(ref err) => {
                tracing::error!("Serialization error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Serialization error".to_string(),
                )
            }
            Error::Io(ref err) => {
                tracing::error!("IO error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Error::Internal(ref err) => {
                tracing::error!("Internal error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
