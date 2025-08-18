//! Error types for the core domain

use thiserror::Error;

/// Core error type for domain operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("UUID parsing error: {0}")]
    UuidParse(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("State transition error: {message}")]
    StateTransition { message: String },

    #[error("Constraint violation: {constraint} - {message}")]
    ConstraintViolation { constraint: String, message: String },

    #[error("Resource exhausted: {resource} - {message}")]
    ResourceExhausted { resource: String, message: String },

    #[error("Operation timeout: {operation} exceeded {timeout_seconds}s")]
    Timeout {
        operation: String,
        timeout_seconds: u64,
    },

    #[error("Permission denied: {action} - {reason}")]
    PermissionDenied { action: String, reason: String },

    #[error("Dependency error: {dependency} - {message}")]
    Dependency { dependency: String, message: String },
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Error::UuidParse(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err.to_string())
    }
}

impl Error {
    /// Create a validation error with a formatted message
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a not found error for a specific entity type and ID
    pub fn not_found<S1: Into<String>, S2: Into<String>>(entity_type: S1, id: S2) -> Self {
        Self::NotFound {
            entity_type: entity_type.into(),
            id: id.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a state transition error
    pub fn state_transition<S: Into<String>>(message: S) -> Self {
        Self::StateTransition {
            message: message.into(),
        }
    }

    /// Create a constraint violation error
    pub fn constraint_violation<S1: Into<String>, S2: Into<String>>(
        constraint: S1,
        message: S2,
    ) -> Self {
        Self::ConstraintViolation {
            constraint: constraint.into(),
            message: message.into(),
        }
    }

    /// Create a resource exhausted error
    pub fn resource_exhausted<S1: Into<String>, S2: Into<String>>(
        resource: S1,
        message: S2,
    ) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S, timeout_seconds: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_seconds,
        }
    }

    /// Create a permission denied error
    pub fn permission_denied<S1: Into<String>, S2: Into<String>>(action: S1, reason: S2) -> Self {
        Self::PermissionDenied {
            action: action.into(),
            reason: reason.into(),
        }
    }

    /// Create a dependency error
    pub fn dependency<S1: Into<String>, S2: Into<String>>(dependency: S1, message: S2) -> Self {
        Self::Dependency {
            dependency: dependency.into(),
            message: message.into(),
        }
    }

    /// Check if this error is a validation error
    pub fn is_validation(&self) -> bool {
        matches!(self, Error::Validation { .. })
    }

    /// Check if this error is a not found error
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound { .. })
    }

    /// Check if this error is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout { .. })
    }

    /// Check if this error is recoverable (client can retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Timeout { .. } | Error::ResourceExhausted { .. } | Error::Dependency { .. }
        )
    }

    /// Get the error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            Error::Validation { .. } => "validation",
            Error::NotFound { .. } => "not_found",
            Error::Serialization(_) => "serialization",
            Error::UuidParse(_) => "uuid_parse",
            Error::Internal(_) => "internal",
            Error::Configuration { .. } => "configuration",
            Error::StateTransition { .. } => "state_transition",
            Error::ConstraintViolation { .. } => "constraint_violation",
            Error::ResourceExhausted { .. } => "resource_exhausted",
            Error::Timeout { .. } => "timeout",
            Error::PermissionDenied { .. } => "permission_denied",
            Error::Dependency { .. } => "dependency",
        }
    }
}

/// Convenience result type for core operations
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let validation_err = Error::validation("Test validation error");
        assert!(validation_err.is_validation());
        assert!(!validation_err.is_not_found());
        assert_eq!(validation_err.category(), "validation");

        let not_found_err = Error::not_found("Agent", "123");
        assert!(not_found_err.is_not_found());
        assert!(!not_found_err.is_validation());
        assert_eq!(not_found_err.category(), "not_found");

        let timeout_err = Error::timeout("task_execution", 300);
        assert!(timeout_err.is_timeout());
        assert!(timeout_err.is_recoverable());
        assert_eq!(timeout_err.category(), "timeout");
    }

    #[test]
    fn test_error_recoverability() {
        let validation_err = Error::validation("Invalid input");
        assert!(!validation_err.is_recoverable());

        let timeout_err = Error::timeout("operation", 60);
        assert!(timeout_err.is_recoverable());

        let resource_err = Error::resource_exhausted("memory", "Out of memory");
        assert!(resource_err.is_recoverable());

        let dependency_err = Error::dependency("database", "Connection failed");
        assert!(dependency_err.is_recoverable());
    }

    #[test]
    fn test_error_from_conversions() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let core_err: Error = json_err.into();
        assert_eq!(core_err.category(), "serialization");

        let uuid_err = uuid::Uuid::parse_str("invalid-uuid").unwrap_err();
        let core_err: Error = uuid_err.into();
        assert_eq!(core_err.category(), "uuid_parse");
    }

    #[test]
    fn test_error_display() {
        let err = Error::constraint_violation("unique_name", "Name already exists");
        let display_str = format!("{}", err);
        assert!(display_str.contains("Constraint violation"));
        assert!(display_str.contains("unique_name"));
        assert!(display_str.contains("Name already exists"));
    }
}
