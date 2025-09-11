use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(ref err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Json(ref err) => (StatusCode::BAD_REQUEST, err.to_string()),
            AppError::Io(ref err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::BadRequest(ref message) => (StatusCode::BAD_REQUEST, message.clone()),
            AppError::NotFound(ref message) => (StatusCode::NOT_FOUND, message.clone()),
            AppError::Internal(ref err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        let body = json!({
            "error": error_message
        });

        (status, axum::Json(body)).into_response()
    }
}

impl From<axum::extract::rejection::JsonRejection> for AppError {
    fn from(rej: axum::extract::rejection::JsonRejection) -> Self {
        AppError::BadRequest(rej.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
