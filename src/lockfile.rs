use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Claude Code IDE lock file format for WebSocket discovery
#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeLockFile {
    #[serde(rename = "authToken")]
    pub auth_token: String,
    #[serde(rename = "ideName")]
    pub ide_name: String,
    pub pid: u32,
    pub transport: String,
    #[serde(rename = "workspaceFolders")]
    pub workspace_folders: Vec<String>,
}

pub struct LockFileManager {
    port: u16,
}

impl LockFileManager {
    pub fn new(_host: String, port: u16) -> Self {
        Self { port }
    }

    /// Get the Claude IDE lock file directory (~/.claude/ide/)
    pub fn get_claude_ide_dir() -> Result<PathBuf, AppError> {
        let home = dirs::home_dir().ok_or_else(|| {
            AppError::BadRequest("Unable to determine home directory".to_string())
        })?;
        Ok(home.join(".claude").join("ide"))
    }

    /// Get the Claude IDE lock file path (~/.claude/ide/{port}.lock)
    pub fn get_claude_lock_file_path(&self) -> Result<PathBuf, AppError> {
        let lock_dir = Self::get_claude_ide_dir()?;
        Ok(lock_dir.join(format!("{}.lock", self.port)))
    }

    /// Server mode: Create or update Claude IDE lock file
    /// This should be called AFTER the server starts listening on the port
    pub fn create_or_update_claude_lock_file(&self) -> Result<String, AppError> {
        let lock_dir = Self::get_claude_ide_dir()?;

        // Create directory if it doesn't exist
        if !lock_dir.exists() {
            fs::create_dir_all(&lock_dir).map_err(|e| {
                AppError::BadRequest(format!("Failed to create Claude IDE directory: {}", e))
            })?;
        }

        let lock_file_path = self.get_claude_lock_file_path()?;
        let current_dir = std::env::current_dir()
            .map_err(|e| AppError::BadRequest(format!("Failed to get current directory: {}", e)))?
            .to_string_lossy()
            .to_string();

        // Check if file exists and read existing workspace folders
        let existing_workspace_folders = if lock_file_path.exists() {
            match self.read_claude_lock_file() {
                Ok(existing_lock) => existing_lock.workspace_folders,
                Err(_) => vec![current_dir.clone()], // If we can't read it, start fresh
            }
        } else {
            vec![current_dir.clone()]
        };

        // Ensure current directory is in workspace folders (no duplicates)
        let mut workspace_folders = existing_workspace_folders;
        if !workspace_folders.contains(&current_dir) {
            workspace_folders.push(current_dir);
        }

        let token = Uuid::new_v4().to_string();
        let lock_file = ClaudeLockFile {
            auth_token: token.clone(),
            ide_name: "Vibe Ensemble MCP".to_string(),
            pid: std::process::id(),
            transport: "ws".to_string(),
            workspace_folders,
        };

        let lock_file_content = serde_json::to_string_pretty(&lock_file)
            .map_err(|e| AppError::BadRequest(format!("Failed to serialize lock file: {}", e)))?;

        fs::write(&lock_file_path, lock_file_content)
            .map_err(|e| AppError::BadRequest(format!("Failed to write lock file: {}", e)))?;

        tracing::info!(
            "Created/updated Claude IDE lock file: {}",
            lock_file_path.display()
        );
        Ok(token)
    }

    /// Read the Claude IDE lock file
    pub fn read_claude_lock_file(&self) -> Result<ClaudeLockFile, AppError> {
        let lock_file_path = self.get_claude_lock_file_path()?;

        if !lock_file_path.exists() {
            return Err(AppError::NotFound(format!(
                "Claude lock file not found: {}",
                lock_file_path.display()
            )));
        }

        let content = fs::read_to_string(&lock_file_path)
            .map_err(|e| AppError::BadRequest(format!("Failed to read Claude lock file: {}", e)))?;

        let lock_file: ClaudeLockFile = serde_json::from_str(&content).map_err(|e| {
            AppError::BadRequest(format!("Failed to parse Claude lock file: {}", e))
        })?;

        Ok(lock_file)
    }

    /// Client mode: Check if Claude IDE lock file exists and validate workspace folder
    /// Returns the auth token if successful, or an error if the lock file doesn't exist
    pub fn validate_claude_lock_file_for_client(&self) -> Result<String, AppError> {
        let lock_file_path = self.get_claude_lock_file_path()?;

        if !lock_file_path.exists() {
            return Err(AppError::NotFound(format!(
                "Claude lock file not found at {}. \
                Please start the vibe-ensemble server first before running --configure-claude-code.",
                lock_file_path.display()
            )));
        }

        let mut lock_file = self.read_claude_lock_file()?;
        let current_dir = std::env::current_dir()
            .map_err(|e| AppError::BadRequest(format!("Failed to get current directory: {}", e)))?
            .to_string_lossy()
            .to_string();

        // Ensure current directory is in workspace folders (no duplicates)
        if !lock_file.workspace_folders.contains(&current_dir) {
            lock_file.workspace_folders.push(current_dir);

            // Write back the updated lock file
            let lock_file_content = serde_json::to_string_pretty(&lock_file).map_err(|e| {
                AppError::BadRequest(format!("Failed to serialize lock file: {}", e))
            })?;

            fs::write(&lock_file_path, lock_file_content)
                .map_err(|e| AppError::BadRequest(format!("Failed to update lock file: {}", e)))?;

            tracing::info!("Updated workspace folders in Claude IDE lock file");
        }

        Ok(lock_file.auth_token)
    }

    /// Clean up the Claude IDE lock file
    pub fn cleanup_claude_lock_file(&self) -> Result<(), AppError> {
        let lock_file_path = self.get_claude_lock_file_path()?;

        if lock_file_path.exists() {
            fs::remove_file(&lock_file_path).map_err(|e| {
                AppError::BadRequest(format!("Failed to remove Claude lock file: {}", e))
            })?;
            tracing::info!("Removed Claude IDE lock file: {}", lock_file_path.display());
        }

        Ok(())
    }

    /// Find a Claude lock file by port (useful for checking if server is running)
    pub fn find_claude_lock_file_by_port(port: u16) -> Result<Option<ClaudeLockFile>, AppError> {
        let lock_dir = Self::get_claude_ide_dir()?;

        if !lock_dir.exists() {
            return Ok(None);
        }

        let lock_file_path = lock_dir.join(format!("{}.lock", port));

        if !lock_file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_file_path)
            .map_err(|e| AppError::BadRequest(format!("Failed to read Claude lock file: {}", e)))?;

        let lock_file: ClaudeLockFile = serde_json::from_str(&content).map_err(|e| {
            AppError::BadRequest(format!("Failed to parse Claude lock file: {}", e))
        })?;

        Ok(Some(lock_file))
    }
}
