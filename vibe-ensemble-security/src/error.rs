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

impl SecurityError {
    /// Get a sanitized error message for external consumption
    /// This prevents leaking sensitive information like database errors, internal paths, etc.
    pub fn public_message(&self) -> String {
        match self {
            SecurityError::AuthenticationFailed(_) => "Authentication failed".to_string(),
            SecurityError::AuthorizationFailed(_) => "Access denied".to_string(),
            SecurityError::JwtError(_) => "Invalid authentication token".to_string(),
            SecurityError::PasswordHashError(_) => "Password processing error".to_string(),
            SecurityError::EncryptionError(_) => "Encryption error".to_string(),
            SecurityError::RateLimitExceeded => "Rate limit exceeded".to_string(),
            SecurityError::InvalidTokenFormat => "Invalid token format".to_string(),
            SecurityError::TokenExpired => "Token expired".to_string(),
            SecurityError::InsufficientPermissions { .. } => "Insufficient permissions".to_string(),
            SecurityError::DatabaseError(_) => "Internal server error".to_string(),
            SecurityError::Internal(_) => "Internal server error".to_string(),
            SecurityError::ConfigurationError(_) => "Configuration error".to_string(),
            SecurityError::SessionError(_) => "Session error".to_string(),
            SecurityError::AuditError(_) => "Audit error".to_string(),
        }
    }

    /// Get detailed error message for internal logging (includes sensitive information)
    pub fn internal_message(&self) -> String {
        self.to_string()
    }

    /// Check if this error should be logged with high severity for security monitoring
    pub fn is_security_sensitive(&self) -> bool {
        matches!(
            self,
            SecurityError::AuthenticationFailed(_)
                | SecurityError::AuthorizationFailed(_)
                | SecurityError::InsufficientPermissions { .. }
                | SecurityError::RateLimitExceeded
                | SecurityError::InvalidTokenFormat
                | SecurityError::TokenExpired
        )
    }

    /// Get appropriate HTTP status code for this error
    pub fn http_status_code(&self) -> u16 {
        match self {
            SecurityError::AuthenticationFailed(_)
            | SecurityError::JwtError(_)
            | SecurityError::InvalidTokenFormat
            | SecurityError::TokenExpired => 401,

            SecurityError::AuthorizationFailed(_)
            | SecurityError::InsufficientPermissions { .. } => 403,

            SecurityError::RateLimitExceeded => 429,

            SecurityError::PasswordHashError(_)
            | SecurityError::EncryptionError(_)
            | SecurityError::DatabaseError(_)
            | SecurityError::Internal(_)
            | SecurityError::ConfigurationError(_)
            | SecurityError::SessionError(_)
            | SecurityError::AuditError(_) => 500,
        }
    }
}

/// Sanitized error response for API endpoints
#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: u16,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ErrorResponse {
    /// Create error response from SecurityError
    pub fn from_security_error(error: &SecurityError) -> Self {
        Self {
            error: error.public_message(),
            status: error.http_status_code(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create generic internal error response
    pub fn internal_error() -> Self {
        Self {
            error: "Internal server error".to_string(),
            status: 500,
            timestamp: chrono::Utc::now(),
        }
    }
}
