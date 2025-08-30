//! File system verification utilities for integration testing
//!
//! This module provides comprehensive file system verification capabilities
//! to validate that workers create the expected files with correct content
//! in their isolated worktrees.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;

/// Verifies file system changes made by workers during integration tests
#[derive(Debug)]
pub struct FileSystemVerifier {
    /// Verification rules and expectations
    rules: VerificationRules,
    /// Cache of file states for performance
    file_cache: HashMap<PathBuf, FileState>,
}

impl FileSystemVerifier {
    /// Creates a new file system verifier
    pub fn new() -> Self {
        Self {
            rules: VerificationRules::default(),
            file_cache: HashMap::new(),
        }
    }

    /// Creates a verifier with custom rules
    pub fn with_rules(rules: VerificationRules) -> Self {
        Self {
            rules,
            file_cache: HashMap::new(),
        }
    }

    /// Verifies that a file exists with the expected content
    pub async fn verify_file_exists(
        &mut self,
        file_path: &Path,
        expected_content: &str,
    ) -> Result<(), FileVerificationError> {
        // Check if file exists
        if !file_path.exists() {
            return Err(FileVerificationError::FileNotFound {
                path: file_path.to_path_buf(),
                expected: true,
            });
        }

        // Check if it's actually a file
        if !file_path.is_file() {
            return Err(FileVerificationError::InvalidFileType {
                path: file_path.to_path_buf(),
                expected_type: FileType::RegularFile,
                actual_type: if file_path.is_dir() {
                    FileType::Directory
                } else {
                    FileType::Other
                },
            });
        }

        // Read and verify content
        let actual_content =
            fs::read_to_string(file_path)
                .await
                .map_err(|e| FileVerificationError::ReadError {
                    path: file_path.to_path_buf(),
                    error: e.to_string(),
                })?;

        if !self
            .rules
            .content_matches(&actual_content, expected_content)
        {
            return Err(FileVerificationError::ContentMismatch {
                path: file_path.to_path_buf(),
                expected: expected_content.to_string(),
                actual: actual_content,
            });
        }

        // Cache the file state
        self.cache_file_state(file_path).await?;

        Ok(())
    }

    /// Verifies that a file does not exist
    pub async fn verify_file_not_exists(
        &self,
        file_path: &Path,
    ) -> Result<(), FileVerificationError> {
        if file_path.exists() {
            return Err(FileVerificationError::FileNotFound {
                path: file_path.to_path_buf(),
                expected: false,
            });
        }
        Ok(())
    }

    /// Verifies that a directory exists with expected structure
    pub async fn verify_directory_structure(
        &mut self,
        dir_path: &Path,
        expected_structure: &DirectoryStructure,
    ) -> Result<(), FileVerificationError> {
        if !dir_path.exists() {
            return Err(FileVerificationError::FileNotFound {
                path: dir_path.to_path_buf(),
                expected: true,
            });
        }

        if !dir_path.is_dir() {
            return Err(FileVerificationError::InvalidFileType {
                path: dir_path.to_path_buf(),
                expected_type: FileType::Directory,
                actual_type: FileType::RegularFile,
            });
        }

        // Check required files
        for required_file in &expected_structure.required_files {
            let file_path = dir_path.join(required_file);
            if !file_path.exists() {
                return Err(FileVerificationError::FileNotFound {
                    path: file_path,
                    expected: true,
                });
            }
        }

        // Check required directories
        for required_dir in &expected_structure.required_directories {
            let subdir_path = dir_path.join(required_dir);
            if !subdir_path.exists() || !subdir_path.is_dir() {
                return Err(FileVerificationError::FileNotFound {
                    path: subdir_path,
                    expected: true,
                });
            }
        }

        // Check forbidden files
        for forbidden_file in &expected_structure.forbidden_files {
            let file_path = dir_path.join(forbidden_file);
            if file_path.exists() {
                return Err(FileVerificationError::ForbiddenFileFound { path: file_path });
            }
        }

        // Recursively check subdirectory structures
        for (subdir_name, subdir_structure) in &expected_structure.subdirectories {
            let subdir_path = dir_path.join(subdir_name);
            Box::pin(self.verify_directory_structure(&subdir_path, subdir_structure)).await?;
        }

        Ok(())
    }

    /// Verifies file permissions (Unix-like systems)
    #[cfg(unix)]
    pub async fn verify_file_permissions(
        &self,
        file_path: &Path,
        expected_mode: u32,
    ) -> Result<(), FileVerificationError> {
        use std::os::unix::fs::PermissionsExt;

        let metadata =
            fs::metadata(file_path)
                .await
                .map_err(|e| FileVerificationError::ReadError {
                    path: file_path.to_path_buf(),
                    error: e.to_string(),
                })?;

        let actual_mode = metadata.permissions().mode() & 0o777;
        if actual_mode != expected_mode {
            return Err(FileVerificationError::PermissionMismatch {
                path: file_path.to_path_buf(),
                expected: expected_mode,
                actual: actual_mode,
            });
        }

        Ok(())
    }

    /// Verifies file size constraints
    pub async fn verify_file_size(
        &self,
        file_path: &Path,
        size_constraint: SizeConstraint,
    ) -> Result<(), FileVerificationError> {
        let metadata =
            fs::metadata(file_path)
                .await
                .map_err(|e| FileVerificationError::ReadError {
                    path: file_path.to_path_buf(),
                    error: e.to_string(),
                })?;

        let actual_size = metadata.len();
        if !size_constraint.is_satisfied(actual_size) {
            return Err(FileVerificationError::SizeMismatch {
                path: file_path.to_path_buf(),
                constraint: size_constraint,
                actual_size,
            });
        }

        Ok(())
    }

    /// Verifies that a file was created within a time window
    pub async fn verify_file_creation_time(
        &self,
        file_path: &Path,
        expected_window: TimeWindow,
    ) -> Result<(), FileVerificationError> {
        let metadata =
            fs::metadata(file_path)
                .await
                .map_err(|e| FileVerificationError::ReadError {
                    path: file_path.to_path_buf(),
                    error: e.to_string(),
                })?;

        let created = metadata
            .created()
            .or_else(|_| metadata.modified())
            .map_err(|e| FileVerificationError::ReadError {
                path: file_path.to_path_buf(),
                error: format!("Failed to get file timestamp: {}", e),
            })?;

        if !expected_window.contains(created) {
            return Err(FileVerificationError::TimeMismatch {
                path: file_path.to_path_buf(),
                expected_window,
                actual_time: created,
            });
        }

        Ok(())
    }

    /// Performs a comprehensive verification of multiple files
    pub async fn verify_batch(
        &mut self,
        verifications: Vec<FileVerification>,
    ) -> Result<BatchVerificationResult, Vec<FileVerificationError>> {
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        for verification in verifications {
            match self.perform_single_verification(verification).await {
                Ok(result) => results.push(result),
                Err(error) => errors.push(error),
            }
        }

        if errors.is_empty() {
            Ok(BatchVerificationResult {
                successful_verifications: results,
                total_time: start_time.elapsed(),
                files_verified: results.len(),
            })
        } else {
            Err(errors)
        }
    }

    /// Caches the current state of a file
    async fn cache_file_state(&mut self, file_path: &Path) -> Result<(), FileVerificationError> {
        let metadata =
            fs::metadata(file_path)
                .await
                .map_err(|e| FileVerificationError::ReadError {
                    path: file_path.to_path_buf(),
                    error: e.to_string(),
                })?;

        let content = if metadata.is_file() && metadata.len() <= self.rules.max_content_size {
            // Only cache content for files smaller than configured limit
            Some(fs::read_to_string(file_path).await.unwrap_or_default())
        } else {
            None
        };

        let state = FileState {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            content,
            is_directory: metadata.is_dir(),
        };

        self.file_cache.insert(file_path.to_path_buf(), state);
        Ok(())
    }

    /// Performs a single verification operation
    async fn perform_single_verification(
        &mut self,
        verification: FileVerification,
    ) -> Result<SingleVerificationResult, FileVerificationError> {
        let start_time = std::time::Instant::now();

        match verification {
            FileVerification::FileExists {
                path,
                expected_content,
            } => {
                self.verify_file_exists(&path, &expected_content).await?;
                Ok(SingleVerificationResult {
                    verification_type: "file_exists".to_string(),
                    file_path: path,
                    duration: start_time.elapsed(),
                })
            }
            FileVerification::FileNotExists { path } => {
                self.verify_file_not_exists(&path).await?;
                Ok(SingleVerificationResult {
                    verification_type: "file_not_exists".to_string(),
                    file_path: path,
                    duration: start_time.elapsed(),
                })
            }
            FileVerification::DirectoryStructure { path, structure } => {
                self.verify_directory_structure(&path, &structure).await?;
                Ok(SingleVerificationResult {
                    verification_type: "directory_structure".to_string(),
                    file_path: path,
                    duration: start_time.elapsed(),
                })
            }
            FileVerification::FileSize { path, constraint } => {
                self.verify_file_size(&path, constraint).await?;
                Ok(SingleVerificationResult {
                    verification_type: "file_size".to_string(),
                    file_path: path,
                    duration: start_time.elapsed(),
                })
            }
        }
    }

    /// Gets the cached file state if available
    pub fn get_cached_state(&self, file_path: &Path) -> Option<&FileState> {
        self.file_cache.get(file_path)
    }

    /// Clears the file cache
    pub fn clear_cache(&mut self) {
        self.file_cache.clear();
    }
}

impl Default for FileSystemVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Rules for file system verification
#[derive(Debug, Clone)]
pub struct VerificationRules {
    /// Whether to ignore whitespace differences in content
    pub ignore_whitespace: bool,
    /// Whether to ignore case differences in content
    pub ignore_case: bool,
    /// Whether to allow extra files not specified in requirements
    pub allow_extra_files: bool,
    /// Maximum file size to read into memory for content verification
    pub max_content_size: u64,
}

impl VerificationRules {
    /// Checks if two content strings match according to the rules
    pub fn content_matches(&self, actual: &str, expected: &str) -> bool {
        let actual_normalized = if self.ignore_whitespace {
            actual
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        } else {
            actual.to_string()
        };

        let expected_normalized = if self.ignore_whitespace {
            expected
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        } else {
            expected.to_string()
        };

        if self.ignore_case {
            actual_normalized.to_lowercase() == expected_normalized.to_lowercase()
        } else {
            actual_normalized == expected_normalized
        }
    }
}

impl Default for VerificationRules {
    fn default() -> Self {
        Self {
            ignore_whitespace: false,
            ignore_case: false,
            allow_extra_files: true,
            max_content_size: 1024 * 1024, // 1MB
        }
    }
}

/// Expected directory structure definition
#[derive(Debug, Clone)]
pub struct DirectoryStructure {
    /// Files that must exist in this directory
    pub required_files: Vec<String>,
    /// Subdirectories that must exist
    pub required_directories: Vec<String>,
    /// Files that must not exist
    pub forbidden_files: Vec<String>,
    /// Expected subdirectory structures
    pub subdirectories: HashMap<String, DirectoryStructure>,
}

impl DirectoryStructure {
    /// Creates a new empty directory structure
    pub fn new() -> Self {
        Self {
            required_files: Vec::new(),
            required_directories: Vec::new(),
            forbidden_files: Vec::new(),
            subdirectories: HashMap::new(),
        }
    }

    /// Adds a required file
    pub fn require_file(mut self, filename: &str) -> Self {
        self.required_files.push(filename.to_string());
        self
    }

    /// Adds a required directory
    pub fn require_directory(mut self, dirname: &str) -> Self {
        self.required_directories.push(dirname.to_string());
        self
    }

    /// Adds a forbidden file
    pub fn forbid_file(mut self, filename: &str) -> Self {
        self.forbidden_files.push(filename.to_string());
        self
    }

    /// Adds a subdirectory structure expectation
    pub fn with_subdirectory(mut self, dirname: &str, structure: DirectoryStructure) -> Self {
        self.subdirectories.insert(dirname.to_string(), structure);
        self
    }
}

impl Default for DirectoryStructure {
    fn default() -> Self {
        Self::new()
    }
}

/// File size constraint
#[derive(Debug, Clone, Copy)]
pub enum SizeConstraint {
    /// Exact size in bytes
    Exact(u64),
    /// Minimum size in bytes
    AtLeast(u64),
    /// Maximum size in bytes
    AtMost(u64),
    /// Size range (min, max) in bytes
    Range(u64, u64),
    /// File must be empty
    Empty,
    /// File must not be empty
    NonEmpty,
}

impl SizeConstraint {
    /// Checks if a file size satisfies this constraint
    pub fn is_satisfied(&self, actual_size: u64) -> bool {
        match self {
            SizeConstraint::Exact(expected) => actual_size == *expected,
            SizeConstraint::AtLeast(min) => actual_size >= *min,
            SizeConstraint::AtMost(max) => actual_size <= *max,
            SizeConstraint::Range(min, max) => actual_size >= *min && actual_size <= *max,
            SizeConstraint::Empty => actual_size == 0,
            SizeConstraint::NonEmpty => actual_size > 0,
        }
    }
}

/// Time window for file creation/modification checks
#[derive(Debug, Clone, Copy)]
pub struct TimeWindow {
    /// Start of the window
    pub start: SystemTime,
    /// End of the window
    pub end: SystemTime,
}

impl TimeWindow {
    /// Creates a new time window
    pub fn new(start: SystemTime, end: SystemTime) -> Self {
        Self { start, end }
    }

    /// Creates a time window from now minus/plus the given duration
    pub fn around_now(tolerance: Duration) -> Self {
        let now = SystemTime::now();
        Self {
            start: now - tolerance,
            end: now + tolerance,
        }
    }

    /// Checks if a time falls within this window
    pub fn contains(&self, time: SystemTime) -> bool {
        time >= self.start && time <= self.end
    }
}

/// Cached file state information
#[derive(Debug, Clone)]
pub struct FileState {
    /// File size in bytes
    pub size: u64,
    /// Last modified time
    pub modified: SystemTime,
    /// File content (if cached)
    pub content: Option<String>,
    /// Whether this is a directory
    pub is_directory: bool,
}

/// Type of file system entity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file
    RegularFile,
    /// Directory
    Directory,
    /// Symbolic link or other special file
    Other,
}

/// Single verification operation definition
#[derive(Debug, Clone)]
pub enum FileVerification {
    /// Verify that a file exists with specific content
    FileExists {
        path: PathBuf,
        expected_content: String,
    },
    /// Verify that a file does not exist
    FileNotExists { path: PathBuf },
    /// Verify directory structure
    DirectoryStructure {
        path: PathBuf,
        structure: DirectoryStructure,
    },
    /// Verify file size
    FileSize {
        path: PathBuf,
        constraint: SizeConstraint,
    },
}

/// Result of a single verification operation
#[derive(Debug, Clone)]
pub struct SingleVerificationResult {
    /// Type of verification performed
    pub verification_type: String,
    /// Path that was verified
    pub file_path: PathBuf,
    /// Time taken for verification
    pub duration: Duration,
}

/// Result of batch verification
#[derive(Debug, Clone)]
pub struct BatchVerificationResult {
    /// Successful verifications
    pub successful_verifications: Vec<SingleVerificationResult>,
    /// Total time for all verifications
    pub total_time: Duration,
    /// Number of files verified
    pub files_verified: usize,
}

/// File verification error types
#[derive(Debug, Clone)]
pub enum FileVerificationError {
    /// File not found (or found when it shouldn't be)
    FileNotFound { path: PathBuf, expected: bool },
    /// Wrong file type (file vs directory)
    InvalidFileType {
        path: PathBuf,
        expected_type: FileType,
        actual_type: FileType,
    },
    /// File content doesn't match expected
    ContentMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    /// Error reading file
    ReadError { path: PathBuf, error: String },
    /// File size constraint violation
    SizeMismatch {
        path: PathBuf,
        constraint: SizeConstraint,
        actual_size: u64,
    },
    /// File permissions mismatch
    PermissionMismatch {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    /// File creation/modification time outside expected window
    TimeMismatch {
        path: PathBuf,
        expected_window: TimeWindow,
        actual_time: SystemTime,
    },
    /// A forbidden file was found
    ForbiddenFileFound { path: PathBuf },
}

impl std::fmt::Display for FileVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileVerificationError::FileNotFound { path, expected } => {
                write!(
                    f,
                    "File {}: {}",
                    path.display(),
                    if *expected {
                        "not found"
                    } else {
                        "unexpectedly exists"
                    }
                )
            }
            FileVerificationError::InvalidFileType {
                path,
                expected_type,
                actual_type,
            } => {
                write!(
                    f,
                    "File {} has wrong type: expected {:?}, got {:?}",
                    path.display(),
                    expected_type,
                    actual_type
                )
            }
            FileVerificationError::ContentMismatch { path, .. } => {
                write!(f, "File {} has incorrect content", path.display())
            }
            FileVerificationError::ReadError { path, error } => {
                write!(f, "Failed to read {}: {}", path.display(), error)
            }
            FileVerificationError::SizeMismatch {
                path,
                constraint,
                actual_size,
            } => {
                write!(
                    f,
                    "File {} size {} doesn't meet constraint {:?}",
                    path.display(),
                    actual_size,
                    constraint
                )
            }
            FileVerificationError::PermissionMismatch {
                path,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "File {} permissions {:o} don't match expected {:o}",
                    path.display(),
                    actual,
                    expected
                )
            }
            FileVerificationError::TimeMismatch { path, .. } => {
                write!(
                    f,
                    "File {} creation time outside expected window",
                    path.display()
                )
            }
            FileVerificationError::ForbiddenFileFound { path } => {
                write!(f, "Forbidden file found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for FileVerificationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use uuid::Uuid;

    async fn create_test_file(
        path: &Path,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_file_exists_verification() {
        let temp_dir = env::temp_dir().join(format!("verifier-test-{}", Uuid::new_v4()));
        let test_file = temp_dir.join("test.txt");
        let content = "Hello, world!";

        create_test_file(&test_file, content).await.unwrap();

        let mut verifier = FileSystemVerifier::new();
        let result = verifier.verify_file_exists(&test_file, content).await;

        assert!(result.is_ok());

        // Test content mismatch
        let wrong_result = verifier
            .verify_file_exists(&test_file, "Wrong content")
            .await;
        assert!(wrong_result.is_err());

        fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_directory_structure_verification() {
        let temp_dir = env::temp_dir().join(format!("verifier-dir-test-{}", Uuid::new_v4()));

        // Create test structure
        create_test_file(&temp_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        create_test_file(&temp_dir.join("subdir/file2.txt"), "content2")
            .await
            .unwrap();
        fs::create_dir_all(&temp_dir.join("empty_dir"))
            .await
            .unwrap();

        let structure = DirectoryStructure::new()
            .require_file("file1.txt")
            .require_directory("subdir")
            .require_directory("empty_dir")
            .forbid_file("forbidden.txt");

        let mut verifier = FileSystemVerifier::new();
        let result = verifier
            .verify_directory_structure(&temp_dir, &structure)
            .await;

        assert!(result.is_ok());

        // Test with forbidden file
        create_test_file(&temp_dir.join("forbidden.txt"), "forbidden")
            .await
            .unwrap();
        let forbidden_result = verifier
            .verify_directory_structure(&temp_dir, &structure)
            .await;
        assert!(forbidden_result.is_err());

        fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_verification() {
        let temp_dir = env::temp_dir().join(format!("verifier-batch-test-{}", Uuid::new_v4()));

        create_test_file(&temp_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        create_test_file(&temp_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let verifications = vec![
            FileVerification::FileExists {
                path: temp_dir.join("file1.txt"),
                expected_content: "content1".to_string(),
            },
            FileVerification::FileExists {
                path: temp_dir.join("file2.txt"),
                expected_content: "content2".to_string(),
            },
            FileVerification::FileNotExists {
                path: temp_dir.join("nonexistent.txt"),
            },
        ];

        let mut verifier = FileSystemVerifier::new();
        let result = verifier.verify_batch(verifications).await;

        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.files_verified, 3);

        fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_size_constraint() {
        let temp_dir = env::temp_dir().join(format!("verifier-size-test-{}", Uuid::new_v4()));
        let test_file = temp_dir.join("test.txt");
        let content = "12345"; // 5 bytes

        create_test_file(&test_file, content).await.unwrap();

        let verifier = FileSystemVerifier::new();

        // Test exact size
        assert!(verifier
            .verify_file_size(&test_file, SizeConstraint::Exact(5))
            .await
            .is_ok());
        assert!(verifier
            .verify_file_size(&test_file, SizeConstraint::Exact(4))
            .await
            .is_err());

        // Test range
        assert!(verifier
            .verify_file_size(&test_file, SizeConstraint::Range(3, 7))
            .await
            .is_ok());
        assert!(verifier
            .verify_file_size(&test_file, SizeConstraint::Range(6, 10))
            .await
            .is_err());

        // Test non-empty
        assert!(verifier
            .verify_file_size(&test_file, SizeConstraint::NonEmpty)
            .await
            .is_ok());

        fs::remove_dir_all(&temp_dir).await.unwrap();
    }

    #[tokio::test]
    async fn test_verification_rules() {
        let rules_ignore_whitespace = VerificationRules {
            ignore_whitespace: true,
            ignore_case: false,
            allow_extra_files: true,
            max_content_size: 1024,
        };

        assert!(rules_ignore_whitespace.content_matches("hello world", "helloworld"));
        assert!(rules_ignore_whitespace.content_matches("  hello  \n  world  ", "helloworld"));
        assert!(!rules_ignore_whitespace.content_matches("hello world", "Hello World"));

        let rules_ignore_case = VerificationRules {
            ignore_whitespace: false,
            ignore_case: true,
            allow_extra_files: true,
            max_content_size: 1024,
        };

        assert!(rules_ignore_case.content_matches("Hello World", "hello world"));
        assert!(!rules_ignore_case.content_matches("hello world", "helloworld"));
    }
}
