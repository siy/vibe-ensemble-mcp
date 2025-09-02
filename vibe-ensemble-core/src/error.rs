//! Error types for the core domain

use thiserror::Error;

/// Core error type for domain operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Multiple validation errors:\n{}", .errors.iter().enumerate().map(|(i, e)| format!("{}. {}", i + 1, e)).collect::<Vec<_>>().join("\n"))]
    MultipleValidationErrors { errors: Vec<String> },

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

    #[error("Execution error: {message}")]
    Execution { message: String },

    #[error("Parsing error: {message}")]
    Parsing { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("Rendering error: {message}")]
    Rendering { message: String },

    #[error("Resource already exists: {resource} with id {id}")]
    AlreadyExists { resource: String, id: String },
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

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io {
            message: err.to_string(),
        }
    }
}

impl Error {
    /// Create a validation error with a formatted message
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a multiple validation errors from a vector of error messages
    pub fn multiple_validation_errors(errors: Vec<String>) -> Self {
        Self::MultipleValidationErrors { errors }
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

    /// Create an execution error
    pub fn execution<S: Into<String>>(message: S) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }

    /// Create a parsing error
    pub fn parsing<S: Into<String>>(message: S) -> Self {
        Self::Parsing {
            message: message.into(),
        }
    }

    /// Create an IO error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a rendering error
    pub fn rendering<S: Into<String>>(message: S) -> Self {
        Self::Rendering {
            message: message.into(),
        }
    }

    /// Create an already-exists error
    pub fn already_exists<S1: Into<String>, S2: Into<String>>(resource: S1, id: S2) -> Self {
        Self::AlreadyExists {
            resource: resource.into(),
            id: id.into(),
        }
    }

    /// Check if this error is a validation error
    pub fn is_validation(&self) -> bool {
        matches!(
            self,
            Error::Validation { .. } | Error::MultipleValidationErrors { .. }
        )
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
            Error::MultipleValidationErrors { .. } => "validation",
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
            Error::Execution { .. } => "execution",
            Error::Parsing { .. } => "parsing",
            Error::Io { .. } => "io",
            Error::Rendering { .. } => "rendering",
            Error::AlreadyExists { .. } => "already_exists",
        }
    }
}

/// Convenience result type for core operations
pub type Result<T> = std::result::Result<T, Error>;

/// Validation error accumulator for collecting multiple validation errors
#[derive(Debug, Default)]
pub struct ValidationErrors {
    errors: Vec<String>,
}

impl ValidationErrors {
    /// Create a new validation error accumulator
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add a validation error message
    pub fn add<S: Into<String>>(&mut self, error: S) {
        self.errors.push(error.into());
    }

    /// Add a result, collecting any validation errors
    pub fn add_result<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.add(error.to_string());
                None
            }
        }
    }

    /// Check if there are any validation errors
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the number of validation errors
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Convert to a result, returning an error if there are any validation issues
    pub fn into_result<T>(self, success_value: T) -> Result<T> {
        if self.errors.is_empty() {
            Ok(success_value)
        } else {
            Err(Error::multiple_validation_errors(self.errors))
        }
    }

    /// Convert to an error if there are validation issues
    pub fn into_error(self) -> Option<Error> {
        if self.errors.is_empty() {
            None
        } else {
            Some(Error::multiple_validation_errors(self.errors))
        }
    }

    /// Get all error messages
    pub fn messages(&self) -> &[String] {
        &self.errors
    }
}

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

    #[test]
    fn test_new_error_categories() {
        assert_eq!(Error::execution("boom").category(), "execution");
        assert_eq!(Error::parsing("bad token").category(), "parsing");
        assert_eq!(Error::io("disk full").category(), "io");
        assert_eq!(Error::rendering("template err").category(), "rendering");
        assert_eq!(
            Error::already_exists("Agent", "123").category(),
            "already_exists"
        );
    }

    #[test]
    fn test_multiple_validation_errors() {
        let errors = vec![
            "Field 'name' is required".to_string(),
            "Field 'endpoint' is invalid".to_string(),
        ];
        let multi_error = Error::multiple_validation_errors(errors);

        assert!(multi_error.is_validation());
        assert_eq!(multi_error.category(), "validation");

        let display_str = format!("{}", multi_error);
        assert!(display_str.contains("Multiple validation errors"));
        assert!(display_str.contains("1. Field 'name' is required"));
        assert!(display_str.contains("2. Field 'endpoint' is invalid"));
    }

    #[test]
    fn test_validation_errors_accumulator() {
        let mut validator = ValidationErrors::new();
        assert!(validator.is_empty());
        assert_eq!(validator.len(), 0);

        validator.add("First error");
        validator.add("Second error");
        assert!(!validator.is_empty());
        assert_eq!(validator.len(), 2);

        // Test converting to error
        let error = validator.into_error().unwrap();
        assert!(error.is_validation());
    }

    #[test]
    fn test_validation_errors_with_results() {
        let mut validator = ValidationErrors::new();

        // Add successful result
        let success_result: Result<String> = Ok("success".to_string());
        let value = validator.add_result(success_result);
        assert_eq!(value, Some("success".to_string()));

        // Add failed result
        let failed_result: Result<String> = Err(Error::validation("test error"));
        let value = validator.add_result(failed_result);
        assert_eq!(value, None);

        assert_eq!(validator.len(), 1);
        assert_eq!(validator.messages()[0], "Validation error: test error");
    }
}
