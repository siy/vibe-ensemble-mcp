//! Core framework utilities for coordinator-worker integration tests

use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;
use uuid::Uuid;

/// Test execution context that tracks resources and state
#[derive(Debug)]
pub struct TestContext {
    /// Unique ID for this test execution
    pub test_id: Uuid,
    /// Base directory for test files
    pub base_dir: PathBuf,
    /// Whether to cleanup resources after test
    pub cleanup_on_drop: bool,
    /// Maximum test execution timeout
    pub timeout: Duration,
}

impl TestContext {
    /// Creates a new test context
    pub fn new(test_name: &str) -> Self {
        let test_id = Uuid::new_v4();
        let base_dir = std::env::temp_dir()
            .join("vibe-ensemble-integration-tests")
            .join(format!("{}_{}", test_name, test_id));

        Self {
            test_id,
            base_dir,
            cleanup_on_drop: true,
            timeout: Duration::from_secs(300), // 5 minutes default timeout
        }
    }

    /// Sets the test timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Disables automatic cleanup for debugging
    pub fn without_cleanup(mut self) -> Self {
        self.cleanup_on_drop = false;
        self
    }

    /// Creates the test directory structure
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tokio::fs::create_dir_all(&self.base_dir).await?;
        Ok(())
    }

    /// Gets a path within the test directory
    pub fn path(&self, relative_path: &str) -> PathBuf {
        self.base_dir.join(relative_path)
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        if self.cleanup_on_drop && self.base_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.base_dir);
        }
    }
}

/// Helper for creating predictable test data
pub struct TestDataBuilder {
    /// Counter for generating unique names
    counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl TestDataBuilder {
    /// Creates a new test data builder
    pub fn new() -> Self {
        Self {
            counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Generates a unique ticket definition
    pub fn create_ticket(&self, title_prefix: &str) -> super::TicketDefinition {
        let id = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        super::TicketDefinition {
            id: Uuid::new_v4(),
            title: format!("{} #{}", title_prefix, id),
            description: format!("Automated test ticket #{}", id),
            priority: "medium".to_string(),
        }
    }

    /// Generates a unique expected file
    pub fn create_expected_file(&self, path: &str, content_template: &str) -> super::ExpectedFile {
        let id = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        super::ExpectedFile {
            path: PathBuf::from(path),
            expected_content: content_template.replace("{id}", &id.to_string()),
        }
    }
}

impl Default for TestDataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility for measuring and reporting test performance
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// Start time of monitoring
    start_time: std::time::Instant,
    /// Phase measurements
    phases: Vec<PhaseMetric>,
    /// Current phase being measured
    current_phase: Option<String>,
}

#[derive(Debug)]
pub struct PhaseMetric {
    /// Name of the phase
    pub name: String,
    /// Duration of the phase
    pub duration: Duration,
    /// When the phase started relative to test start
    pub start_offset: Duration,
}

impl PerformanceMonitor {
    /// Creates a new performance monitor
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            phases: Vec::new(),
            current_phase: None,
        }
    }

    /// Starts measuring a new phase
    pub fn start_phase(&mut self, name: &str) {
        // End current phase if one is active
        if let Some(current) = &self.current_phase {
            self.end_phase(current);
        }
        
        self.current_phase = Some(name.to_string());
    }

    /// Ends the current phase
    pub fn end_phase(&mut self, expected_name: &str) {
        if let Some(current) = &self.current_phase {
            if current == expected_name {
                let now = std::time::Instant::now();
                let total_duration = now.duration_since(self.start_time);
                
                // Calculate phase duration (approximate)
                let phase_start = if let Some(last_phase) = self.phases.last() {
                    last_phase.start_offset + last_phase.duration
                } else {
                    Duration::from_secs(0)
                };
                
                let phase_duration = total_duration - phase_start;
                
                self.phases.push(PhaseMetric {
                    name: current.clone(),
                    duration: phase_duration,
                    start_offset: phase_start,
                });
                
                self.current_phase = None;
            }
        }
    }

    /// Gets all measured phases
    pub fn phases(&self) -> &[PhaseMetric] {
        &self.phases
    }

    /// Gets the total elapsed time
    pub fn total_elapsed(&self) -> Duration {
        std::time::Instant::now().duration_since(self.start_time)
    }

    /// Generates a performance report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Performance Report ===\n");
        report.push_str(&format!("Total Duration: {:?}\n", self.total_elapsed()));
        report.push_str("Phase Breakdown:\n");
        
        for phase in &self.phases {
            let percentage = (phase.duration.as_secs_f64() / self.total_elapsed().as_secs_f64()) * 100.0;
            report.push_str(&format!(
                "  {}: {:?} ({:.1}%)\n", 
                phase.name, 
                phase.duration,
                percentage
            ));
        }
        
        report
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Assertion helpers for integration tests
pub struct TestAssertions;

impl TestAssertions {
    /// Asserts that a file exists with expected content
    pub async fn assert_file_content(
        path: &std::path::Path, 
        expected_content: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !path.exists() {
            return Err(format!("File does not exist: {}", path.display()).into());
        }

        let actual_content = tokio::fs::read_to_string(path).await?;
        if actual_content.trim() != expected_content.trim() {
            return Err(format!(
                "File content mismatch in {}:\nExpected:\n{}\nActual:\n{}", 
                path.display(),
                expected_content,
                actual_content
            ).into());
        }

        Ok(())
    }

    /// Asserts that a directory contains expected files
    pub async fn assert_directory_structure(
        dir: &std::path::Path,
        expected_files: &[String]
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !dir.exists() || !dir.is_dir() {
            return Err(format!("Directory does not exist: {}", dir.display()).into());
        }

        for expected_file in expected_files {
            let file_path = dir.join(expected_file);
            if !file_path.exists() {
                return Err(format!("Expected file missing: {}", file_path.display()).into());
            }
        }

        Ok(())
    }

    /// Asserts that a git worktree is in expected state
    pub async fn assert_git_worktree_state(
        worktree_path: &std::path::Path,
        expected_branch: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = tokio::process::Command::new("git")
            .args(&["branch", "--show-current"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            return Err("Failed to get git branch".into());
        }

        let current_branch = String::from_utf8(output.stdout)?
            .trim()
            .to_string();

        if current_branch != expected_branch {
            return Err(format!(
                "Git branch mismatch. Expected: {}, Actual: {}", 
                expected_branch, 
                current_branch
            ).into());
        }

        Ok(())
    }

    /// Asserts that MCP tool call succeeded
    pub fn assert_mcp_success(response: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(error) = response.get("error") {
            return Err(format!("MCP tool call failed: {}", error).into());
        }

        if response.get("result").is_none() {
            return Err("MCP tool call missing result".into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_creation_and_cleanup() {
        let ctx = TestContext::new("test_framework");
        ctx.initialize().await.unwrap();
        
        assert!(ctx.base_dir.to_string_lossy().contains("test_framework"));
        
        // Create a test file
        let test_file = ctx.path("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();
        assert!(test_file.exists());
        
        // Context should clean up automatically on drop
        drop(ctx);
    }

    #[test]
    fn test_data_builder_uniqueness() {
        let builder = TestDataBuilder::new();
        
        let ticket1 = builder.create_ticket("Test Ticket");
        let ticket2 = builder.create_ticket("Test Ticket");
        
        assert_ne!(ticket1.id, ticket2.id);
        assert!(ticket1.title.contains("#0"));
        assert!(ticket2.title.contains("#1"));
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let mut monitor = PerformanceMonitor::new();
        
        monitor.start_phase("setup");
        tokio::time::sleep(Duration::from_millis(10)).await;
        monitor.end_phase("setup");
        
        monitor.start_phase("execution");
        tokio::time::sleep(Duration::from_millis(20)).await;
        monitor.end_phase("execution");
        
        let phases = monitor.phases();
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].name, "setup");
        assert_eq!(phases[1].name, "execution");
        
        let report = monitor.generate_report();
        assert!(report.contains("Performance Report"));
        assert!(report.contains("setup"));
        assert!(report.contains("execution"));
    }

    #[tokio::test]
    async fn test_assertions() {
        let temp_dir = std::env::temp_dir().join("test_assertions");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();
        
        // Test file content assertion
        let test_file = temp_dir.join("test.txt");
        tokio::fs::write(&test_file, "expected content").await.unwrap();
        
        TestAssertions::assert_file_content(&test_file, "expected content")
            .await
            .unwrap();
        
        // Test directory structure assertion
        let sub_file = temp_dir.join("sub.txt");
        tokio::fs::write(&sub_file, "sub content").await.unwrap();
        
        TestAssertions::assert_directory_structure(
            &temp_dir,
            &["test.txt".to_string(), "sub.txt".to_string()]
        ).await.unwrap();
        
        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.unwrap();
    }
}