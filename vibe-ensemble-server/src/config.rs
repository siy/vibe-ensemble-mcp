//! Configuration management for the server

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub mcp: McpConfig,
    pub web: WebConfig,
    pub logging: LoggingConfig,
}

/// Server-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: Option<u32>,
    pub migrate_on_startup: bool,
}

/// MCP protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub protocol_version: String,
    pub heartbeat_interval: u64,
    pub max_message_size: usize,
}

/// Web interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub static_files_path: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
            },
            database: DatabaseConfig {
                url: "sqlite:./vibe_ensemble.db".to_string(),
                max_connections: Some(10),
                migrate_on_startup: true,
            },
            mcp: McpConfig {
                protocol_version: "1.0.0".to_string(),
                heartbeat_interval: 30,
                max_message_size: 1024 * 1024, // 1MB
            },
            web: WebConfig {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 8081,
                static_files_path: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
            },
        }
    }
}

impl Config {
    /// Load configuration from environment and config files
    pub fn load() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name("config/local").required(false))
            .add_source(config::Environment::with_prefix("VIBE_ENSEMBLE"))
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 8080)?
            .set_default("database.url", "sqlite:./vibe_ensemble.db")?
            .set_default("database.migrate_on_startup", true)?
            .set_default("mcp.protocol_version", "1.0.0")?
            .set_default("mcp.heartbeat_interval", 30)?
            .set_default("mcp.max_message_size", 1048576)?
            .set_default("web.enabled", true)?
            .set_default("web.host", "127.0.0.1")?
            .set_default("web.port", 8081)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .build()?;

        settings.try_deserialize()
    }

    /// Get the server socket address
    pub fn server_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .expect("Invalid server address")
    }

    /// Get the web interface socket address
    pub fn web_addr(&self) -> SocketAddr {
        format!("{}:{}", self.web.host, self.web.port)
            .parse()
            .expect("Invalid web address")
    }
}
