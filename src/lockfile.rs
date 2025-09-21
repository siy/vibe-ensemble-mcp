use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub port: u16,
    pub host: String,
    pub token: String,
    pub pid: u32,
    pub endpoints: LockFileEndpoints,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFileEndpoints {
    pub mcp: String,
    pub sse: String,
    pub ws: String,
}

pub struct LockFileManager {
    port: u16,
    host: String,
}

impl LockFileManager {
    pub fn new(host: String, port: u16) -> Self {
        Self { port, host }
    }

    pub fn get_lock_dir() -> Result<PathBuf, AppError> {
        let home = dirs::home_dir().ok_or_else(|| {
            AppError::BadRequest("Unable to determine home directory".to_string())
        })?;
        Ok(home.join(".vibe-ensemble-mcp"))
    }

    pub fn get_lock_file_path(&self) -> Result<PathBuf, AppError> {
        let lock_dir = Self::get_lock_dir()?;
        Ok(lock_dir.join(format!("connection-{}.json", self.port)))
    }

    pub fn create_lock_file(&self) -> Result<String, AppError> {
        let lock_dir = Self::get_lock_dir()?;

        // Create directory if it doesn't exist
        if !lock_dir.exists() {
            fs::create_dir_all(&lock_dir).map_err(|e| {
                AppError::BadRequest(format!("Failed to create lock directory: {}", e))
            })?;
        }

        let token = Uuid::new_v4().to_string();
        let lock_file = LockFile {
            port: self.port,
            host: self.host.clone(),
            token: token.clone(),
            pid: std::process::id(),
            endpoints: LockFileEndpoints {
                mcp: format!("http://{}:{}/mcp", self.host, self.port),
                sse: format!("http://{}:{}/sse", self.host, self.port),
                ws: format!("ws://{}:{}/ws", self.host, self.port),
            },
        };

        let lock_file_path = self.get_lock_file_path()?;
        let lock_file_content = serde_json::to_string_pretty(&lock_file)
            .map_err(|e| AppError::BadRequest(format!("Failed to serialize lock file: {}", e)))?;

        fs::write(&lock_file_path, lock_file_content)
            .map_err(|e| AppError::BadRequest(format!("Failed to write lock file: {}", e)))?;

        tracing::info!("Created lock file: {}", lock_file_path.display());
        Ok(token)
    }

    pub fn read_lock_file(&self) -> Result<LockFile, AppError> {
        let lock_file_path = self.get_lock_file_path()?;

        if !lock_file_path.exists() {
            return Err(AppError::NotFound(format!(
                "Lock file not found: {}",
                lock_file_path.display()
            )));
        }

        let content = fs::read_to_string(&lock_file_path)
            .map_err(|e| AppError::BadRequest(format!("Failed to read lock file: {}", e)))?;

        let lock_file: LockFile = serde_json::from_str(&content)
            .map_err(|e| AppError::BadRequest(format!("Failed to parse lock file: {}", e)))?;

        Ok(lock_file)
    }

    pub fn cleanup_lock_file(&self) -> Result<(), AppError> {
        let lock_file_path = self.get_lock_file_path()?;

        if lock_file_path.exists() {
            fs::remove_file(&lock_file_path)
                .map_err(|e| AppError::BadRequest(format!("Failed to remove lock file: {}", e)))?;
            tracing::info!("Removed lock file: {}", lock_file_path.display());
        }

        Ok(())
    }

    pub fn find_server_by_port(port: u16) -> Result<Option<LockFile>, AppError> {
        let lock_dir = Self::get_lock_dir()?;

        if !lock_dir.exists() {
            return Ok(None);
        }

        let lock_file_path = lock_dir.join(format!("connection-{}.json", port));

        if !lock_file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_file_path)
            .map_err(|e| AppError::BadRequest(format!("Failed to read lock file: {}", e)))?;

        let lock_file: LockFile = serde_json::from_str(&content)
            .map_err(|e| AppError::BadRequest(format!("Failed to parse lock file: {}", e)))?;

        Ok(Some(lock_file))
    }
}
