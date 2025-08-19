//! Security-related error types

use thiserror::Error;

/// Security-related errors
#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("JWT token error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Password hashing error: {0}")]
    PasswordHashError(#[from] bcrypt::BcryptError),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid token format")]
    InvalidTokenFormat,

    #[error("Token expired")]
    TokenExpired,

    #[error("Insufficient permissions: required {required}, have {current}")]
    InsufficientPermissions { required: String, current: String },

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Internal security error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Audit error: {0}")]
    AuditError(String),
}

/// Result type for security operations
pub type SecurityResult<T> = Result<T, SecurityError>;
