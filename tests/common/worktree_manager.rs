//! Git worktree management for workspace isolation in integration tests
//!
//! This module provides utilities for creating, managing, and cleaning up
//! git worktrees to ensure workers have isolated workspaces during testing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Manager for git worktrees used in integration testing
#[derive(Debug)]
pub struct GitWorktreeManager {
    /// Base repository path
    base_repo: PathBuf,
    /// Active worktrees (name -> path mapping)
    active_worktrees: HashMap<String, PathBuf>,
    /// Worktree counter for unique naming
    counter: std::sync::atomic::AtomicUsize,
}

impl GitWorktreeManager {
    /// Creates a new worktree manager
    pub fn new(base_repo: PathBuf) -> Self {
        Self {
            base_repo,
            active_worktrees: HashMap::new(),
            counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Creates a new git worktree for isolated work
    pub async fn create_worktree(&mut self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let count = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let unique_name = format!("{}-{}", name, count);
        let branch_name = format!("integration-test/{}", unique_name);
        
        // Create worktree directory path
        let worktree_path = self.base_repo
            .parent()
            .unwrap_or(&self.base_repo)
            .join(format!("worktree-{}", unique_name));

        // Create the worktree with a new branch
        let output = tokio::process::Command::new("git")
            .args(&[
                "worktree", "add", 
                "-b", &branch_name,
                worktree_path.to_str().unwrap(),
                "HEAD"
            ])
            .current_dir(&self.base_repo)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to create worktree '{}': {}", unique_name, error).into());
        }

        // Track the worktree
        self.active_worktrees.insert(unique_name.clone(), worktree_path.clone());

        // Set up git config in the worktree
        tokio::process::Command::new("git")
            .args(&["config", "user.name", "Integration Test Worker"])
            .current_dir(&worktree_path)
            .output()
            .await?;

        tokio::process::Command::new("git")
            .args(&["config", "user.email", "worker@vibe-ensemble.local"])
            .current_dir(&worktree_path)
            .output()
            .await?;

        Ok(worktree_path)
    }

    /// Lists all active worktrees
    pub fn active_worktrees(&self) -> &HashMap<String, PathBuf> {
        &self.active_worktrees
    }

    /// Gets a specific worktree path by name
    pub fn get_worktree(&self, name: &str) -> Option<&PathBuf> {
        self.active_worktrees.get(name)
    }

    /// Commits changes in a worktree
    pub async fn commit_changes(
        &self,
        worktree_path: &Path,
        message: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Add all changes
        let add_output = tokio::process::Command::new("git")
            .args(&["add", "."])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !add_output.status.success() {
            let error = String::from_utf8_lossy(&add_output.stderr);
            return Err(format!("Failed to add changes: {}", error).into());
        }

        // Check if there are changes to commit
        let status_output = tokio::process::Command::new("git")
            .args(&["diff", "--cached", "--quiet"])
            .current_dir(worktree_path)
            .output()
            .await?;

        // If git diff --cached --quiet returns 0, there are no changes
        if status_output.status.success() {
            return Ok(()); // No changes to commit
        }

        // Commit changes
        let commit_output = tokio::process::Command::new("git")
            .args(&["commit", "-m", message])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !commit_output.status.success() {
            let error = String::from_utf8_lossy(&commit_output.stderr);
            return Err(format!("Failed to commit changes: {}", error).into());
        }

        Ok(())
    }

    /// Gets the current branch name in a worktree
    pub async fn get_current_branch(
        &self,
        worktree_path: &Path
    ) -> Result<String, Box<dyn std::error::Error>> {
        let output = tokio::process::Command::new("git")
            .args(&["branch", "--show-current"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to get current branch: {}", error).into());
        }

        let branch = String::from_utf8(output.stdout)?
            .trim()
            .to_string();

        Ok(branch)
    }

    /// Lists files in a worktree
    pub async fn list_files(
        &self,
        worktree_path: &Path
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(worktree_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Ok(relative_path) = path.strip_prefix(worktree_path) {
                    files.push(relative_path.to_path_buf());
                }
            } else if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                // Recursively list files in subdirectories (skip .git)
                let subfiles = self.list_files_recursive(&path, worktree_path).await?;
                files.extend(subfiles);
            }
        }

        files.sort();
        Ok(files)
    }

    /// Recursively lists files in a directory
    async fn list_files_recursive(
        &self,
        dir: &Path,
        base_path: &Path
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Ok(relative_path) = path.strip_prefix(base_path) {
                    files.push(relative_path.to_path_buf());
                }
            } else if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                let subfiles = self.list_files_recursive(&path, base_path).await?;
                files.extend(subfiles);
            }
        }

        Ok(files)
    }

    /// Gets the git status of a worktree
    pub async fn get_status(
        &self,
        worktree_path: &Path
    ) -> Result<GitWorktreeStatus, Box<dyn std::error::Error>> {
        let output = tokio::process::Command::new("git")
            .args(&["status", "--porcelain"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to get git status: {}", error).into());
        }

        let status_text = String::from_utf8(output.stdout)?;
        let mut status = GitWorktreeStatus::new();

        for line in status_text.lines() {
            if line.len() >= 3 {
                let status_code = &line[0..2];
                let file_path = &line[3..];
                
                match status_code {
                    "??" => status.untracked.push(PathBuf::from(file_path)),
                    " M" => status.modified.push(PathBuf::from(file_path)),
                    "M " => status.staged.push(PathBuf::from(file_path)),
                    "A " => status.added.push(PathBuf::from(file_path)),
                    "D " => status.deleted.push(PathBuf::from(file_path)),
                    " D" => status.deleted.push(PathBuf::from(file_path)),
                    _ => status.other.push((status_code.to_string(), PathBuf::from(file_path))),
                }
            }
        }

        Ok(status)
    }

    /// Cleans up a specific worktree
    pub async fn cleanup_worktree(&mut self, worktree_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Find the worktree name by path
        let worktree_name = self.active_worktrees
            .iter()
            .find(|(_, path)| path.as_path() == worktree_path)
            .map(|(name, _)| name.clone());

        if let Some(name) = worktree_name {
            // Remove the worktree
            let output = tokio::process::Command::new("git")
                .args(&["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
                .current_dir(&self.base_repo)
                .output()
                .await?;

            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                // Don't fail if worktree is already removed
                if !error.contains("not a working tree") {
                    return Err(format!("Failed to remove worktree: {}", error).into());
                }
            }

            // Remove from tracking
            self.active_worktrees.remove(&name);
        }

        // Also try to remove the directory if it still exists
        if worktree_path.exists() {
            tokio::fs::remove_dir_all(worktree_path).await.ok();
        }

        Ok(())
    }

    /// Cleans up all active worktrees
    pub async fn cleanup_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let worktree_paths: Vec<PathBuf> = self.active_worktrees.values().cloned().collect();
        
        for path in worktree_paths {
            self.cleanup_worktree(&path).await?;
        }

        Ok(())
    }

    /// Prunes any stale worktree references
    pub async fn prune_worktrees(&self) -> Result<(), Box<dyn std::error::Error>> {
        let output = tokio::process::Command::new("git")
            .args(&["worktree", "prune"])
            .current_dir(&self.base_repo)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to prune worktrees: {}", error).into());
        }

        Ok(())
    }
}

impl Drop for GitWorktreeManager {
    fn drop(&mut self) {
        // Best effort cleanup - spawn a task to cleanup worktrees
        let worktree_paths: Vec<PathBuf> = self.active_worktrees.values().cloned().collect();
        let base_repo = self.base_repo.clone();
        
        tokio::spawn(async move {
            for path in worktree_paths {
                let _ = tokio::process::Command::new("git")
                    .args(&["worktree", "remove", "--force", path.to_str().unwrap()])
                    .current_dir(&base_repo)
                    .output()
                    .await;
                
                if path.exists() {
                    let _ = tokio::fs::remove_dir_all(&path).await;
                }
            }
        });
    }
}

/// Git status information for a worktree
#[derive(Debug, Clone)]
pub struct GitWorktreeStatus {
    /// Untracked files
    pub untracked: Vec<PathBuf>,
    /// Modified files (unstaged)
    pub modified: Vec<PathBuf>,
    /// Staged files
    pub staged: Vec<PathBuf>,
    /// Added files
    pub added: Vec<PathBuf>,
    /// Deleted files
    pub deleted: Vec<PathBuf>,
    /// Other status changes
    pub other: Vec<(String, PathBuf)>,
}

impl GitWorktreeStatus {
    /// Creates a new empty status
    pub fn new() -> Self {
        Self {
            untracked: Vec::new(),
            modified: Vec::new(),
            staged: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            other: Vec::new(),
        }
    }

    /// Checks if the worktree is clean (no changes)
    pub fn is_clean(&self) -> bool {
        self.untracked.is_empty() 
            && self.modified.is_empty() 
            && self.staged.is_empty()
            && self.added.is_empty()
            && self.deleted.is_empty()
            && self.other.is_empty()
    }

    /// Gets total number of changed files
    pub fn total_changes(&self) -> usize {
        self.untracked.len() 
            + self.modified.len() 
            + self.staged.len()
            + self.added.len()
            + self.deleted.len()
            + self.other.len()
    }
}

impl Default for GitWorktreeStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility for creating test scenarios with multiple worktrees
pub struct WorktreeTestScenario {
    /// Manager instance
    manager: GitWorktreeManager,
    /// Scenario name
    name: String,
    /// Created worktrees for this scenario
    worktrees: Vec<PathBuf>,
}

impl WorktreeTestScenario {
    /// Creates a new test scenario
    pub async fn new(name: &str, base_repo: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            manager: GitWorktreeManager::new(base_repo),
            name: name.to_string(),
            worktrees: Vec::new(),
        })
    }

    /// Adds a worktree for a specific worker
    pub async fn add_worker_worktree(&mut self, worker_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let worktree_path = self.manager.create_worktree(&format!("{}-{}", self.name, worker_name)).await?;
        self.worktrees.push(worktree_path.clone());
        Ok(worktree_path)
    }

    /// Gets all worktrees created for this scenario
    pub fn worktrees(&self) -> &[PathBuf] {
        &self.worktrees
    }

    /// Cleans up all worktrees for this scenario
    pub async fn cleanup(mut self) -> Result<(), Box<dyn std::error::Error>> {
        for worktree in &self.worktrees {
            self.manager.cleanup_worktree(worktree).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    async fn setup_test_repo() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let test_dir = env::temp_dir().join(format!("worktree-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&test_dir).await?;

        // Initialize git repo
        tokio::process::Command::new("git")
            .args(&["init"])
            .current_dir(&test_dir)
            .output()
            .await?;

        // Set up git config
        tokio::process::Command::new("git")
            .args(&["config", "user.name", "Test"])
            .current_dir(&test_dir)
            .output()
            .await?;

        tokio::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&test_dir)
            .output()
            .await?;

        // Create initial commit
        tokio::fs::write(test_dir.join("README.md"), "# Test Repo").await?;
        tokio::process::Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(&test_dir)
            .output()
            .await?;

        tokio::process::Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&test_dir)
            .output()
            .await?;

        Ok(test_dir)
    }

    #[tokio::test]
    async fn test_worktree_creation() {
        let test_repo = setup_test_repo().await.unwrap();
        let mut manager = GitWorktreeManager::new(test_repo.clone());

        let worktree_path = manager.create_worktree("test-worker").await.unwrap();
        
        assert!(worktree_path.exists());
        assert!(worktree_path.join("README.md").exists());

        let branch = manager.get_current_branch(&worktree_path).await.unwrap();
        assert!(branch.starts_with("integration-test/"));

        manager.cleanup_worktree(&worktree_path).await.unwrap();
        assert!(!worktree_path.exists());

        // Cleanup
        tokio::fs::remove_dir_all(&test_repo).await.unwrap();
    }

    #[tokio::test]
    async fn test_file_operations() {
        let test_repo = setup_test_repo().await.unwrap();
        let mut manager = GitWorktreeManager::new(test_repo.clone());

        let worktree_path = manager.create_worktree("file-test").await.unwrap();

        // Create a test file
        let test_file = worktree_path.join("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();

        // List files
        let files = manager.list_files(&worktree_path).await.unwrap();
        assert!(files.iter().any(|f| f.file_name().unwrap() == "test.txt"));

        // Check status
        let status = manager.get_status(&worktree_path).await.unwrap();
        assert!(!status.is_clean());
        assert!(status.untracked.iter().any(|f| f.file_name().unwrap() == "test.txt"));

        // Commit changes
        manager.commit_changes(&worktree_path, "Add test file").await.unwrap();

        // Status should be clean now
        let status_after = manager.get_status(&worktree_path).await.unwrap();
        assert!(status_after.is_clean());

        manager.cleanup_worktree(&worktree_path).await.unwrap();
        tokio::fs::remove_dir_all(&test_repo).await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_worktrees() {
        let test_repo = setup_test_repo().await.unwrap();
        let mut manager = GitWorktreeManager::new(test_repo.clone());

        let worktree1 = manager.create_worktree("worker1").await.unwrap();
        let worktree2 = manager.create_worktree("worker2").await.unwrap();

        assert_eq!(manager.active_worktrees().len(), 2);
        assert!(worktree1.exists());
        assert!(worktree2.exists());
        assert_ne!(worktree1, worktree2);

        // Each worktree should have its own branch
        let branch1 = manager.get_current_branch(&worktree1).await.unwrap();
        let branch2 = manager.get_current_branch(&worktree2).await.unwrap();
        assert_ne!(branch1, branch2);

        manager.cleanup_all().await.unwrap();
        assert!(manager.active_worktrees().is_empty());
        assert!(!worktree1.exists());
        assert!(!worktree2.exists());

        tokio::fs::remove_dir_all(&test_repo).await.unwrap();
    }

    #[tokio::test]
    async fn test_scenario_workflow() {
        let test_repo = setup_test_repo().await.unwrap();
        let mut scenario = WorktreeTestScenario::new("test-scenario", test_repo.clone()).await.unwrap();

        let worker1_path = scenario.add_worker_worktree("backend").await.unwrap();
        let worker2_path = scenario.add_worker_worktree("frontend").await.unwrap();

        assert_eq!(scenario.worktrees().len(), 2);
        assert!(worker1_path.exists());
        assert!(worker2_path.exists());

        scenario.cleanup().await.unwrap();
        assert!(!worker1_path.exists());
        assert!(!worker2_path.exists());

        tokio::fs::remove_dir_all(&test_repo).await.unwrap();
    }
}