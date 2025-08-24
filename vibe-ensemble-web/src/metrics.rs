//! System metrics collection for dashboard monitoring

use crate::templates::{StorageMetrics, SystemMetrics};
//use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::time::Instant;
use vibe_ensemble_storage::StorageManager;

/// System metrics collector
pub struct MetricsCollector {
    start_time: Instant,
    storage: Arc<StorageManager>,
}

impl MetricsCollector {
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self {
            start_time: Instant::now(),
            storage,
        }
    }

    /// Collect current system metrics
    pub async fn collect_system_metrics(&self) -> SystemMetrics {
        let uptime_seconds = self.start_time.elapsed().as_secs();

        // Use cross-platform system information collection
        let (cpu_usage, memory_info, disk_info) = self.collect_system_info().await;

        SystemMetrics {
            cpu_usage_percent: cpu_usage,
            memory_usage_mb: memory_info.0,
            memory_total_mb: memory_info.1,
            disk_usage_mb: disk_info.0,
            disk_total_mb: disk_info.1,
            uptime_seconds,
            active_connections: 0, // TODO: Track actual connections
        }
    }

    /// Collect storage health metrics
    pub async fn collect_storage_metrics(&self) -> StorageMetrics {
        // Get database statistics from storage manager
        let stats = match self.storage.health_check().await {
            Ok(_) => {
                // Database is healthy, collect metrics
                let database_size = self.estimate_database_size().await;

                StorageMetrics {
                    database_size_mb: database_size,
                    total_queries: 0, // TODO: Add query counter to storage manager
                    avg_query_time_ms: 0, // TODO: Add query timing to storage manager
                    active_connections: 1, // Simple estimation for SQLite
                    max_connections: 10, // From default config
                }
            }
            Err(_) => {
                // Database issues, return minimal metrics
                StorageMetrics {
                    database_size_mb: 0,
                    total_queries: 0,
                    avg_query_time_ms: 0,
                    active_connections: 0,
                    max_connections: 10,
                }
            }
        };

        stats
    }

    /// Cross-platform system information collection
    async fn collect_system_info(&self) -> (f64, (u64, u64), (u64, u64)) {
        // Basic cross-platform system info collection
        // In a production system, you might use a crate like sysinfo for more detailed metrics

        let cpu_usage = self.estimate_cpu_usage().await;
        let memory_info = self.get_memory_info().await;
        let disk_info = self.get_disk_info().await;

        (cpu_usage, memory_info, disk_info)
    }

    /// Estimate CPU usage (simplified approach)
    async fn estimate_cpu_usage(&self) -> f64 {
        // Simple CPU usage estimation
        // In production, use proper system monitoring crates
        let load = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1) as f64;

        // Simulate some basic load calculation (placeholder)
        (load * 10.0).min(100.0)
    }

    /// Get memory information
    async fn get_memory_info(&self) -> (u64, u64) {
        // Cross-platform memory info
        // This is a simplified implementation
        // In production, use system monitoring crates like sysinfo

        #[cfg(unix)]
        {
            if let Ok(output) = tokio::process::Command::new("free")
                .arg("-m")
                .output()
                .await
            {
                if let Ok(output_str) = String::from_utf8(output.stdout) {
                    return self.parse_free_output(&output_str);
                }
            }
        }

        // Fallback values for development/unsupported platforms
        (512, 4096) // 512MB used out of 4GB total
    }

    /// Get disk information
    async fn get_disk_info(&self) -> (u64, u64) {
        // Cross-platform disk info
        // This is a simplified implementation

        #[cfg(unix)]
        {
            if let Ok(output) = tokio::process::Command::new("df")
                .arg("-m")
                .arg(".")
                .output()
                .await
            {
                if let Ok(output_str) = String::from_utf8(output.stdout) {
                    return self.parse_df_output(&output_str);
                }
            }
        }

        #[cfg(windows)]
        {
            // Windows implementation would use different commands
            // For now, use fallback values
        }

        // Fallback values
        (2048, 10240) // 2GB used out of 10GB total
    }

    /// Parse free command output (Linux/Unix)
    #[cfg(unix)]
    fn parse_free_output(&self, output: &str) -> (u64, u64) {
        for line in output.lines() {
            if line.starts_with("Mem:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let total = parts[1].parse::<u64>().unwrap_or(4096);
                    let used = parts[2].parse::<u64>().unwrap_or(512);
                    return (used, total);
                }
            }
        }
        (512, 4096) // Fallback
    }

    /// Parse df command output (Linux/Unix)
    #[cfg(unix)]
    fn parse_df_output(&self, output: &str) -> (u64, u64) {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() >= 2 {
            let parts: Vec<&str> = lines[1].split_whitespace().collect();
            if parts.len() >= 4 {
                let total = parts[1].parse::<u64>().unwrap_or(10240);
                let used = parts[2].parse::<u64>().unwrap_or(2048);
                return (used, total);
            }
        }
        (2048, 10240) // Fallback
    }

    /// Estimate database size
    async fn estimate_database_size(&self) -> u64 {
        // For SQLite, we could check the file size
        // For other databases, we'd need different approaches

        // Simple estimation - this could be improved
        match tokio::fs::metadata("vibe_ensemble.db").await {
            Ok(metadata) => (metadata.len() / 1024 / 1024).max(1), // Convert to MB
            Err(_) => 1,                                           // 1MB default
        }
    }
}
