//! Configuration management for the server

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing::warn;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub mcp: McpConfig,
    pub web: WebConfig,
    pub logging: LoggingConfig,
    pub monitoring: MonitoringConfig,
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
    pub session_timeout: u64,
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

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_host: String,
    pub metrics_port: u16,
    pub health_host: String,
    pub health_port: u16,
    pub tracing_enabled: bool,
    pub jaeger_endpoint: Option<String>,
    pub alerting_enabled: bool,
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
                session_timeout: 300,          // 5 minutes
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
            monitoring: MonitoringConfig {
                enabled: true,
                metrics_host: "127.0.0.1".to_string(),
                metrics_port: 9090,
                health_host: "127.0.0.1".to_string(),
                health_port: 8090,
                tracing_enabled: true,
                jaeger_endpoint: None,
                alerting_enabled: true,
            },
        }
    }
}

/// Get database URL with environment variable support and platform-appropriate defaults
fn get_default_database_url() -> String {
    // First check for standard DATABASE_URL environment variable
    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        return database_url;
    }

    // Then check VIBE_ENSEMBLE prefixed variable
    if let Ok(database_url) = std::env::var("VIBE_ENSEMBLE_DATABASE_URL") {
        return database_url;
    }

    // Fall back to platform-appropriate default SQLite path
    #[cfg(target_os = "windows")]
    let base_dir = dirs::data_dir();
    #[cfg(not(target_os = "windows"))]
    let base_dir = dirs::data_local_dir();

    const FALLBACK: &str = "sqlite:./vibe_ensemble.db";

    if let Some(data_dir) = base_dir {
        let app_data_dir = data_dir.join("vibe-ensemble");
        if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
            warn!("Failed to create data directory {:?}: {}", app_data_dir, e);
            return FALLBACK.to_string();
        }
        let db_file = app_data_dir.join("vibe_ensemble.db");

        // Use simple absolute path without URL encoding to avoid SQLite issues with %20 encoding
        // SQLx can handle absolute paths with spaces natively
        format!("sqlite:{}", db_file.display())
    } else {
        FALLBACK.to_string()
    }
}

impl Config {
    /// Load configuration from environment and config files with security validation
    pub fn load() -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder();

        // Only add config files if they exist to avoid directory errors
        if std::path::Path::new("config/default.toml").exists() {
            builder = builder.add_source(config::File::with_name("config/default").required(false));
        }
        if std::path::Path::new("config/local.toml").exists() {
            builder = builder.add_source(config::File::with_name("config/local").required(false));
        }

        let settings = builder
            .add_source(config::Environment::with_prefix("VIBE_ENSEMBLE"))
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 8080)?
            .set_default("database.url", get_default_database_url())?
            .set_default("database.migrate_on_startup", true)?
            .set_default("mcp.protocol_version", "1.0.0")?
            .set_default("mcp.heartbeat_interval", 30)?
            .set_default("mcp.max_message_size", 1048576)?
            .set_default("mcp.session_timeout", 300)?
            .set_default("web.enabled", true)?
            .set_default("web.host", "127.0.0.1")?
            .set_default("web.port", 8081)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .set_default("monitoring.enabled", true)?
            .set_default("monitoring.metrics_host", "127.0.0.1")?
            .set_default("monitoring.metrics_port", 9090)?
            .set_default("monitoring.health_host", "127.0.0.1")?
            .set_default("monitoring.health_port", 8090)?
            .set_default("monitoring.tracing_enabled", true)?
            .set_default("monitoring.alerting_enabled", true)?
            .build()?;

        let config: Config = settings.try_deserialize()?;

        // Perform security validation and warnings
        config.validate_security_settings()?;

        Ok(config)
    }

    /// Load configuration from a specific file path with security validation
    pub fn load_from_file(config_path: &str) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(config_path).required(true))
            .add_source(config::Environment::with_prefix("VIBE_ENSEMBLE"))
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 8080)?
            .set_default("database.url", get_default_database_url())?
            .set_default("database.migrate_on_startup", true)?
            .set_default("mcp.protocol_version", "1.0.0")?
            .set_default("mcp.heartbeat_interval", 30)?
            .set_default("mcp.max_message_size", 1048576)?
            .set_default("mcp.session_timeout", 300)?
            .set_default("web.enabled", true)?
            .set_default("web.host", "127.0.0.1")?
            .set_default("web.port", 8081)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .set_default("monitoring.enabled", true)?
            .set_default("monitoring.metrics_host", "127.0.0.1")?
            .set_default("monitoring.metrics_port", 9090)?
            .set_default("monitoring.health_host", "127.0.0.1")?
            .set_default("monitoring.health_port", 8090)?
            .set_default("monitoring.tracing_enabled", true)?
            .set_default("monitoring.alerting_enabled", true)?
            .build()?;

        let config: Config = settings.try_deserialize()?;

        // Perform security validation and warnings
        config.validate_security_settings()?;

        Ok(config)
    }

    /// Validate configuration for security concerns and provide warnings
    pub fn validate_security_settings(&self) -> Result<(), config::ConfigError> {
        // Check for 0.0.0.0 binding in production
        self.check_external_binding(&self.server.host, "server", self.server.port);
        self.check_external_binding(&self.web.host, "web interface", self.web.port);
        self.check_external_binding(
            &self.monitoring.metrics_host,
            "metrics endpoint",
            self.monitoring.metrics_port,
        );
        self.check_external_binding(
            &self.monitoring.health_host,
            "health endpoint",
            self.monitoring.health_port,
        );

        // Validate database configuration
        self.validate_database_config()?;

        // Check for insecure settings
        self.validate_message_size_limits();
        self.validate_logging_configuration();

        Ok(())
    }

    /// Check if a host binding is potentially insecure
    fn check_external_binding(&self, host: &str, service_name: &str, port: u16) {
        match host {
            "0.0.0.0" => {
                warn!(
                    "⚠️  SECURITY WARNING: {} is bound to 0.0.0.0:{} (all interfaces). \
                     This exposes the service to external networks. \
                     For production use, bind to specific interfaces (e.g., 127.0.0.1 for local only).",
                    service_name, port
                );
            }
            host if !host.starts_with("127.0.0.1") && !host.starts_with("localhost") => {
                warn!(
                    "⚠️  SECURITY NOTICE: {} is bound to {}:{} (external interface). \
                     Ensure proper firewall rules and authentication are in place.",
                    service_name, host, port
                );
            }
            _ => {
                // Localhost binding is secure by default
            }
        }
    }

    /// Validate database configuration for security
    fn validate_database_config(&self) -> Result<(), config::ConfigError> {
        let url = &self.database.url;

        // Check for production database without SSL
        if url.starts_with("postgres://") && !url.contains("sslmode") {
            warn!(
                "⚠️  DATABASE SECURITY: PostgreSQL connection without explicit SSL mode. \
                 Consider adding '?sslmode=require' to the connection string for production use."
            );
        }

        // Check for file-based databases in production
        if url.starts_with("sqlite:") {
            warn!(
                "💡 PRODUCTION TIP: Using SQLite database. \
                 For production deployments with multiple instances, consider PostgreSQL for better concurrency."
            );
        }

        // Warn about auto-migration in production
        if self.database.migrate_on_startup {
            warn!(
                "💡 PRODUCTION TIP: Auto-migration is enabled. \
                 For production, consider running migrations separately and setting migrate_on_startup=false."
            );
        }

        Ok(())
    }

    /// Validate message size limits
    fn validate_message_size_limits(&self) {
        const MAX_RECOMMENDED_SIZE: usize = 10 * 1024 * 1024; // 10MB
        const MIN_RECOMMENDED_SIZE: usize = 64 * 1024; // 64KB

        if self.mcp.max_message_size > MAX_RECOMMENDED_SIZE {
            warn!(
                "⚠️  PERFORMANCE WARNING: MCP max message size ({} bytes) exceeds recommended limit of {} bytes. \
                 Large messages may impact performance.",
                self.mcp.max_message_size, MAX_RECOMMENDED_SIZE
            );
        }

        if self.mcp.max_message_size < MIN_RECOMMENDED_SIZE {
            warn!(
                "⚠️  CONFIGURATION WARNING: MCP max message size ({} bytes) is below recommended minimum of {} bytes. \
                 This may cause message truncation.",
                self.mcp.max_message_size, MIN_RECOMMENDED_SIZE
            );
        }
    }

    /// Validate logging configuration
    fn validate_logging_configuration(&self) {
        match self.logging.level.as_str() {
            "trace" | "debug" => {
                warn!(
                    "💡 PERFORMANCE TIP: Logging level '{}' provides detailed information but may impact performance. \
                     Consider 'info' or 'warn' for production.",
                    self.logging.level
                );
            }
            "error" => {
                warn!(
                    "💡 MONITORING TIP: Logging level 'error' may hide important information. \
                     Consider 'warn' or 'info' for better observability."
                );
            }
            _ => {} // info, warn are good defaults
        }
    }

    /// Check if the configuration appears to be for production use
    pub fn is_production_config(&self) -> bool {
        // Heuristics to detect production configuration
        self.server.host != "127.0.0.1"
            || self.database.url.starts_with("postgres://")
            || !self.database.migrate_on_startup
            || self.logging.format == "json"
    }

    /// Print configuration summary for startup
    pub fn print_startup_summary(&self) {
        println!("🚀 Vibe Ensemble MCP Server Starting");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        println!("📡 API Server:");
        println!(
            "   • Listening on {}:{}",
            self.server.host, self.server.port
        );
        println!(
            "   • Health check: http://{}:{}/health",
            self.server.host, self.server.port
        );
        println!(
            "   • Status endpoint: http://{}:{}/status",
            self.server.host, self.server.port
        );

        if self.web.enabled {
            println!();
            println!("🌐 Web Dashboard:");
            println!(
                "   • Dashboard: http://{}:{}/dashboard",
                self.web.host, self.web.port
            );
            println!("   • Real-time system monitoring available");
        }

        if self.monitoring.enabled {
            println!();
            println!("📊 Monitoring:");
            println!(
                "   • Health: http://{}:{}",
                self.monitoring.health_host, self.monitoring.health_port
            );
            println!(
                "   • Metrics: http://{}:{}",
                self.monitoring.metrics_host, self.monitoring.metrics_port
            );
        }

        println!();
        println!("🗄️  Database:");
        let db_type = if self.database.url.starts_with("postgres://") {
            "PostgreSQL (production ready)"
        } else if self.database.url.starts_with("mysql://") {
            "MySQL (production ready)"
        } else {
            "SQLite (development/single-user)"
        };
        println!("   • Type: {}", db_type);
        println!(
            "   • Max connections: {}",
            self.database.max_connections.unwrap_or(10)
        );
        println!(
            "   • Auto-migration: {}",
            if self.database.migrate_on_startup {
                "enabled"
            } else {
                "disabled"
            }
        );

        println!();
        if self.is_production_config() {
            println!("🔒 Production Mode (external interfaces detected)");
            println!("   ⚠️  Review security warnings above");
            println!("   📖 See docs/security-best-practices.md");
        } else {
            println!("🏠 Development Mode (localhost only)");
            println!("   💡 For production, see config/production.toml");
        }

        println!();
        println!("✨ Quick Start:");
        println!(
            "   • Health check: curl http://{}:{}/health",
            self.server.host, self.server.port
        );
        if self.web.enabled {
            println!(
                "   • Open dashboard: http://{}:{}/dashboard",
                self.web.host, self.web.port
            );
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
    }

    /// Get the server socket address
    pub fn server_addr(&self) -> SocketAddr {
        parse_host_port(&self.server.host, self.server.port)
    }

    /// Get the web interface socket address
    pub fn web_addr(&self) -> SocketAddr {
        parse_host_port(&self.web.host, self.web.port)
    }
}

/// Helper function to parse host and port into SocketAddr, handling IPv6 addresses correctly
fn parse_host_port(host: &str, port: u16) -> SocketAddr {
    let addr = if host.contains(':') && !host.starts_with('[') {
        format!("[{}]:{}", host, port)
    } else {
        format!("{}:{}", host, port)
    };
    addr.parse().expect("Invalid socket address")
}
