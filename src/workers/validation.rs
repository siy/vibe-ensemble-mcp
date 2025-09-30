use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::warn;

/// Input validation for worker process spawning
pub struct WorkerInputValidator;

impl WorkerInputValidator {
    /// Validate and canonicalize project path to prevent path traversal attacks
    pub fn validate_project_path(path: &str) -> Result<PathBuf> {
        // First check if path exists
        let path_obj = Path::new(path);
        if !path_obj.exists() {
            return Err(anyhow::anyhow!("Project path does not exist: {}", path));
        }

        // Canonicalize to resolve any .. or symlinks
        let canonical_path = std::fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize project path: {}", path))?;

        // Ensure path is absolute
        if !canonical_path.is_absolute() {
            return Err(anyhow::anyhow!(
                "Project path must be absolute: {}",
                canonical_path.display()
            ));
        }

        // Ensure it's a directory
        if !canonical_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Project path must be a directory: {}",
                canonical_path.display()
            ));
        }

        Ok(canonical_path)
    }

    /// Validate prompt content size and check for suspicious patterns
    pub fn validate_prompt_content(field_name: &str, content: &str, max_size: usize) -> Result<()> {
        if content.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        if content.len() > max_size {
            return Err(anyhow::anyhow!(
                "{} exceeds maximum size: {} > {} bytes",
                field_name,
                content.len(),
                max_size
            ));
        }

        // Check for null bytes which can cause issues
        if content.contains('\0') {
            return Err(anyhow::anyhow!("{} contains null bytes", field_name));
        }

        // Warn about potentially suspicious patterns (but don't reject)
        let suspicious_patterns = [
            ("command substitution", "$("),
            ("backticks", "`"),
            ("shell variable", "${"),
        ];

        for (name, pattern) in suspicious_patterns {
            if content.contains(pattern) {
                warn!(
                    "Suspicious pattern detected in {}: {} (pattern: {})",
                    field_name, name, pattern
                );
            }
        }

        Ok(())
    }

    /// Validate ticket ID format
    pub fn validate_ticket_id(ticket_id: &str) -> Result<()> {
        if ticket_id.is_empty() {
            return Err(anyhow::anyhow!("Ticket ID cannot be empty"));
        }

        if ticket_id.len() > 128 {
            return Err(anyhow::anyhow!(
                "Ticket ID too long: {} > 128 characters",
                ticket_id.len()
            ));
        }

        // Validate format: alphanumeric, hyphens, underscores only
        if !ticket_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow::anyhow!(
                "Invalid ticket ID format: must contain only alphanumeric characters, hyphens, and underscores"
            ));
        }

        Ok(())
    }

    /// Validate worker ID format
    pub fn validate_worker_id(worker_id: &str) -> Result<()> {
        if worker_id.is_empty() {
            return Err(anyhow::anyhow!("Worker ID cannot be empty"));
        }

        if worker_id.len() > 256 {
            return Err(anyhow::anyhow!(
                "Worker ID too long: {} > 256 characters",
                worker_id.len()
            ));
        }

        // Validate format: alphanumeric, hyphens, underscores, colons only
        if !worker_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':')
        {
            return Err(anyhow::anyhow!(
                "Invalid worker ID format: must contain only alphanumeric characters, hyphens, underscores, and colons"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_validate_ticket_id() {
        // Valid IDs
        assert!(WorkerInputValidator::validate_ticket_id("ticket-123").is_ok());
        assert!(WorkerInputValidator::validate_ticket_id("ticket_abc_123").is_ok());
        assert!(WorkerInputValidator::validate_ticket_id("abc123").is_ok());

        // Invalid IDs
        assert!(WorkerInputValidator::validate_ticket_id("").is_err());
        assert!(WorkerInputValidator::validate_ticket_id("ticket@123").is_err());
        assert!(WorkerInputValidator::validate_ticket_id("ticket 123").is_err());
        assert!(WorkerInputValidator::validate_ticket_id(&"a".repeat(129)).is_err());
    }

    #[test]
    fn test_validate_prompt_content() {
        // Valid content
        assert!(WorkerInputValidator::validate_prompt_content("test", "hello world", 1000).is_ok());

        // Empty content
        assert!(WorkerInputValidator::validate_prompt_content("test", "", 1000).is_err());

        // Too large
        assert!(WorkerInputValidator::validate_prompt_content("test", "hello", 3).is_err());

        // Null bytes
        assert!(
            WorkerInputValidator::validate_prompt_content("test", "hello\0world", 1000).is_err()
        );
    }

    #[test]
    fn test_validate_project_path() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_validate_path");
        let _ = fs::create_dir(&temp_dir);

        // Valid path
        assert!(WorkerInputValidator::validate_project_path(temp_dir.to_str().unwrap()).is_ok());

        // Non-existent path
        assert!(WorkerInputValidator::validate_project_path("/nonexistent/path/12345").is_err());

        // Clean up
        let _ = fs::remove_dir(&temp_dir);
    }
}
