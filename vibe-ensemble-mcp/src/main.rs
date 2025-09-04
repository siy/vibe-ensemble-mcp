//! Vibe Ensemble MCP Server - Claude Code Companion
//!
//! WebSocket MCP server for coordinating multiple Claude Code instances.
//! Features WebSocket transport, SQLite database, and local web dashboard.

use clap::Parser;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vibe_ensemble_core::orchestration::{McpServerConfig, WorkerManager, WorkerOutputConfig};
use vibe_ensemble_mcp::{
    server::{CoordinationServices, McpServer},
    transport::WebSocketServer,
    Error,
};

/// CLI for Vibe Ensemble MCP Server - Claude Code Companion
#[derive(Parser)]
#[command(name = "vibe-ensemble")]
#[command(about = "MCP server for coordinating multiple Claude Code instances")]
#[command(version)]
struct Cli {
    /// Database file path (default: .vibe-ensemble/data.db)
    /// Environment variable: VIBE_ENSEMBLE_DB_PATH
    #[arg(long = "db-path")]
    db_path: Option<String>,

    /// Web dashboard host (default: 127.0.0.1)
    /// Environment variable: VIBE_ENSEMBLE_WEB_HOST
    #[arg(long)]
    web_host: Option<String>,

    /// Web dashboard port (default: 8080)  
    /// Environment variable: VIBE_ENSEMBLE_WEB_PORT
    #[arg(long)]
    web_port: Option<u16>,

    /// Logging level: trace, debug, info, warn, error (default: info)
    /// Environment variable: VIBE_ENSEMBLE_LOG_LEVEL
    #[arg(long)]
    log_level: Option<String>,

    /// Path to log files directory (default: ./.vibe-ensemble/logs/)
    /// Environment variable: VIBE_ENSEMBLE_LOG_PATH
    #[arg(long)]
    log_path: Option<String>,

    /// Maximum database connections (default: 10)
    /// Environment variable: VIBE_ENSEMBLE_MAX_CONNECTIONS
    #[arg(long)]
    max_connections: Option<u32>,

    /// Disable database migrations on startup
    /// Environment variable: VIBE_ENSEMBLE_NO_MIGRATE
    #[arg(long)]
    no_migrate: bool,

    /// Run web server only (no MCP transport)
    /// Environment variable: VIBE_ENSEMBLE_WEB_ONLY
    #[arg(long)]
    web_only: bool,

    /// Run MCP server only (no web dashboard)
    /// Environment variable: VIBE_ENSEMBLE_MCP_ONLY
    #[arg(long)]
    mcp_only: bool,

    /// WebSocket MCP server host (default: 127.0.0.1)
    /// Environment variable: VIBE_ENSEMBLE_MCP_HOST
    #[arg(long)]
    mcp_host: Option<String>,

    /// WebSocket MCP server port (default: 8081)
    /// Environment variable: VIBE_ENSEMBLE_MCP_PORT
    #[arg(long)]
    mcp_port: Option<u16>,

    /// Deprecated: Use --log-level=debug instead
    #[arg(long, hide = true)]
    debug: bool,

    /// Deprecated: Use --db-path instead
    #[arg(long, hide = true)]
    database: Option<String>,

    /// Enable worker output logging to files
    /// Environment variable: VIBE_ENSEMBLE_LOG_WORKER_OUTPUT
    #[arg(long)]
    log_worker_output: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Determine log path with precedence: CLI > env > default
    let log_path = cli
        .log_path
        .or_else(|| env::var("VIBE_ENSEMBLE_LOG_PATH").ok())
        .unwrap_or_else(|| {
            // Default to ./.vibe-ensemble/logs/ (current directory)
            let current_dir =
                std::env::current_dir().expect("Could not determine current directory");
            let log_dir = current_dir.join(".vibe-ensemble").join("logs");
            log_dir.display().to_string()
        });

    // Create log directory if it doesn't exist
    let log_dir = Path::new(&log_path);
    if let Err(e) = fs::create_dir_all(log_dir) {
        eprintln!(
            "Warning: Could not create log directory {}: {}",
            log_dir.display(),
            e
        );
    }

    // Determine worker output logging
    let log_worker_output = cli.log_worker_output
        || env::var("VIBE_ENSEMBLE_LOG_WORKER_OUTPUT")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

    if log_worker_output {
        info!(
            "Worker output logging enabled - outputs will be saved to {}",
            log_dir.display()
        );
    }

    // Determine log level with precedence: CLI > env > default
    let log_level = cli
        .log_level
        .or_else(|| env::var("VIBE_ENSEMBLE_LOG_LEVEL").ok())
        .or_else(|| {
            if cli.debug {
                Some("debug".to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "info".to_string());

    let log_filter = match log_level.to_lowercase().as_str() {
        "trace" => "vibe_ensemble_mcp=trace,vibe_ensemble_web=trace,vibe_ensemble_storage=trace,vibe_ensemble_core=trace",
        "debug" => "vibe_ensemble_mcp=debug,vibe_ensemble_web=debug,vibe_ensemble_storage=debug,vibe_ensemble_core=debug", 
        "info" => "vibe_ensemble_mcp=info,vibe_ensemble_web=info,vibe_ensemble_storage=info",
        "warn" => "vibe_ensemble_mcp=warn,vibe_ensemble_web=warn,vibe_ensemble_storage=warn",
        "error" => "vibe_ensemble_mcp=error,vibe_ensemble_web=error,vibe_ensemble_storage=error",
        _ => {
            eprintln!("Warning: Invalid log level '{}', using 'info'", log_level);
            "vibe_ensemble_mcp=info,vibe_ensemble_web=info,vibe_ensemble_storage=info"
        }
    };

    let log_path = PathBuf::from(&log_path);

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_path) {
        eprintln!(
            "Warning: Failed to create log directory '{}': {}",
            log_path.display(),
            e
        );
        eprintln!("Falling back to stderr-only logging");

        // Fallback to stderr-only logging
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| log_filter.into()),
            )
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    } else {
        // Setup dual logging: file + stderr
        let log_file = log_path.join("vibe-ensemble.log");
        let file_appender = tracing_appender::rolling::daily(&log_path, "vibe-ensemble.log");
        let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

        // Keep the guard alive for the duration of the program
        std::mem::forget(_guard);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| log_filter.into()),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(false),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(file_writer)
                    .with_target(false)
                    .with_ansi(false),
            )
            .init();

        info!("Logging to file: {}", log_file.display());
    }

    info!("Starting Vibe Ensemble MCP Server - Claude Code Companion");
    info!("Log directory: {}", log_dir.display());
    if log_worker_output {
        info!("Worker output logging: enabled");
    } else {
        info!("Worker output logging: disabled (use --log-worker-output to enable)");
    }

    // Determine database path with precedence: CLI > env var > deprecated CLI > legacy env > default
    let database_url = cli
        .db_path
        .or_else(|| env::var("VIBE_ENSEMBLE_DB_PATH").ok())
        .or(cli.database) // Backward compatibility
        .or_else(|| env::var("DATABASE_URL").ok()) // Legacy support
        .unwrap_or_else(|| {
            // Default to ./.vibe-ensemble/data.db (current directory)
            let current_dir =
                std::env::current_dir().expect("Could not determine current directory");
            let vibe_dir = current_dir.join(".vibe-ensemble");
            std::fs::create_dir_all(&vibe_dir).expect("Could not create .vibe-ensemble directory");
            format!("sqlite:{}", vibe_dir.join("data.db").display())
        });

    info!("Using database: {}", mask_database_path(&database_url));

    // Determine configuration values with environment variable support
    let max_connections = cli
        .max_connections
        .or_else(|| {
            env::var("VIBE_ENSEMBLE_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(10);

    let no_migrate = cli.no_migrate
        || env::var("VIBE_ENSEMBLE_NO_MIGRATE")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

    let web_only = cli.web_only
        || env::var("VIBE_ENSEMBLE_WEB_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

    let mcp_only = cli.mcp_only
        || env::var("VIBE_ENSEMBLE_MCP_ONLY")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

    if web_only && mcp_only {
        eprintln!("Cannot enable both --web-only and --mcp-only.");
        return Err(anyhow::anyhow!("conflicting flags: web-only and mcp-only"));
    }

    // Determine MCP WebSocket server configuration
    let mcp_host = cli
        .mcp_host
        .or_else(|| env::var("VIBE_ENSEMBLE_MCP_HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let mcp_port = cli
        .mcp_port
        .or_else(|| {
            env::var("VIBE_ENSEMBLE_MCP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(WebSocketServer::DEFAULT_PORT);

    info!(
        "Transport configuration: websocket={}:{}",
        mcp_host, mcp_port
    );

    // Create database configuration with smart defaults
    let db_config = vibe_ensemble_storage::manager::DatabaseConfig {
        url: database_url,
        max_connections: Some(max_connections),
        migrate_on_startup: !no_migrate,
        performance_config: None,
    };

    // Initialize storage manager
    info!("Initializing storage manager...");
    let storage_manager = Arc::new(
        vibe_ensemble_storage::StorageManager::new(&db_config)
            .await
            .map_err(|e| {
                error!("Failed to initialize storage manager: {}", e);
                e
            })?,
    );

    info!("Database connection established");

    // Note: Migrations are now handled in StorageManager::new() if migrate_on_startup is true

    // Start embedded web dashboard with environment variable support
    let web_host = cli
        .web_host
        .or_else(|| env::var("VIBE_ENSEMBLE_WEB_HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let web_port = cli
        .web_port
        .or_else(|| {
            env::var("VIBE_ENSEMBLE_WEB_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(8080);

    // Check for port conflicts and provide helpful error handling
    let web_addr = format!("{}:{}", web_host, web_port);
    if let Err(e) = tokio::net::TcpListener::bind(&web_addr).await {
        match e.kind() {
            std::io::ErrorKind::AddrInUse => {
                error!("Port {} is already in use. Please choose a different port with --web-port or stop the conflicting service.", web_port);
                return Err(anyhow::anyhow!("Port {} already in use", web_port));
            }
            std::io::ErrorKind::PermissionDenied => {
                error!("Permission denied binding to {}. Try using a port above 1024 or run with appropriate privileges.", web_addr);
                return Err(anyhow::anyhow!(
                    "Permission denied for address {}",
                    web_addr
                ));
            }
            _ => {
                error!("Failed to bind to web address {}: {}", web_addr, e);
                return Err(anyhow::anyhow!("Failed to bind to {}: {}", web_addr, e));
            }
        }
    }

    info!(
        "Starting embedded web dashboard on http://{}:{}",
        web_host, web_port
    );

    let web_config = vibe_ensemble_web::server::WebConfig {
        enabled: true,
        host: web_host.clone(),
        port: web_port,
    };

    let web_server =
        vibe_ensemble_web::server::WebServer::new(web_config, storage_manager.clone()).await?;

    // Start web server in background with enhanced error handling
    let mut web_handle = {
        let web_server = web_server;
        let host_port = format!("{}:{}", web_host, web_port);
        tokio::spawn(async move {
            info!("Web dashboard successfully started on http://{}", host_port);
            if let Err(e) = web_server.run().await {
                error!("Web server error on {}: {}", host_port, e);
            }
            info!("Web dashboard on {} has shut down", host_port);
        })
    };

    // Initialize MCP server components
    let agent_service = storage_manager.agent_service();
    let issue_service = storage_manager.issue_service();
    let message_service = storage_manager.message_service();
    let knowledge_service = storage_manager.knowledge_service();

    // Create coordination service
    let coordination_service = Arc::new(vibe_ensemble_storage::services::CoordinationService::new(
        storage_manager.agents(),
        storage_manager.issues(),
        storage_manager.messages(),
        storage_manager.projects(),
    ));

    info!("Services initialized");

    // Create coordination services bundle
    let coordination_services = CoordinationServices::new(
        agent_service,
        issue_service,
        message_service,
        coordination_service,
        knowledge_service,
    );

    // Create MCP server with coordination services
    let mut server = McpServer::with_coordination(coordination_services);

    // Initialize worker manager for Claude Code process management
    info!("Initializing worker manager...");
    let mcp_config = McpServerConfig {
        host: mcp_host.clone(),
        port: mcp_port,
    };

    let worker_output_config = WorkerOutputConfig {
        enabled: log_worker_output,
        log_directory: if log_worker_output {
            Some(log_path.clone())
        } else {
            None
        },
    };

    let worker_manager = Arc::new(WorkerManager::new(mcp_config, worker_output_config));
    server = server.with_worker_manager(worker_manager.clone());

    info!("MCP server with worker management initialized successfully");

    // Note: message_buffer_size was used for stdio transport only
    // WebSocket transport uses its own internal buffering

    // Handle web-only mode with signal handling
    if web_only {
        info!("Running in web-only mode - MCP transport disabled");
        info!(
            "Web dashboard is available on http://{}:{}",
            web_host, web_port
        );
        info!("Press Ctrl+C to stop the server");

        // Wait for shutdown signal or web server completion
        tokio::select! {
            result = &mut web_handle => {
                if let Err(e) = result {
                    error!("Web server task failed: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal in web-only mode");
                web_handle.abort();
                // Give web server time to shut down gracefully
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        info!("Web-only server shutdown completed");
        return Ok(());
    }

    // Handle MCP-only mode
    if mcp_only {
        info!("Running in MCP-only mode - web dashboard disabled");
        web_handle.abort(); // Stop web server since we don't need it
    }

    // Start MCP transport with WebSocket
    info!(
        "Starting MCP server with WebSocket transport on {}:{}",
        mcp_host, mcp_port
    );
    run_websocket_transport(server, mcp_host, mcp_port, &mut web_handle, mcp_only).await?;

    // Step 2: Shutdown worker manager
    info!("Shutting down worker manager...");
    if let Err(e) = worker_manager.shutdown_all().await {
        error!("Error shutting down worker manager: {}", e);
    } else {
        info!("Worker manager shutdown completed");
    }

    // Step 3: Shutdown web server gracefully
    info!("Shutting down web dashboard...");
    web_handle.abort();

    // Give web server time to shut down gracefully
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Step 4: Final cleanup and status
    info!("All services shut down successfully");
    info!("Vibe Ensemble MCP Server shutdown completed");
    Ok(())
}

/// Run MCP server with WebSocket transport (multi-agent mode)
async fn run_websocket_transport(
    server: McpServer,
    mcp_host: String,
    mcp_port: u16,
    _web_handle: &mut tokio::task::JoinHandle<()>,
    _mcp_only: bool,
) -> anyhow::Result<()> {
    // Create WebSocket server
    let ws_server = WebSocketServer::new(mcp_host, mcp_port);
    let bind_address = ws_server.bind_address();

    // Check for port conflicts
    if let Err(e) = tokio::net::TcpListener::bind(&bind_address).await {
        match e.kind() {
            std::io::ErrorKind::AddrInUse => {
                error!("MCP port {} is already in use. Please choose a different port with --mcp-port or stop the conflicting service.", mcp_port);
                return Err(anyhow::anyhow!("MCP port {} already in use", mcp_port));
            }
            std::io::ErrorKind::PermissionDenied => {
                error!("Permission denied binding to MCP {}. Try using a port above 1024 or run with appropriate privileges.", bind_address);
                return Err(anyhow::anyhow!(
                    "Permission denied for MCP address {}",
                    bind_address
                ));
            }
            _ => {
                error!("Failed to bind to MCP address {}: {}", bind_address, e);
                return Err(anyhow::anyhow!(
                    "Failed to bind to MCP {}: {}",
                    bind_address,
                    e
                ));
            }
        }
    }

    info!("WebSocket MCP server listening on {}", bind_address);

    // Start WebSocket server and handle connections
    let mut connection_rx = ws_server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start WebSocket server: {}", e))?;

    let mut active_connections = 0u64;
    let server_start = std::time::Instant::now();
    info!("WebSocket MCP server running - Press Ctrl+C to stop");

    // Main WebSocket server loop
    loop {
        tokio::select! {
            connection_result = connection_rx.recv() => {
                match connection_result {
                    Some(Ok(mut transport)) => {
                        active_connections += 1;
                        let connection_id = active_connections;
                        info!("New WebSocket connection #{} established", connection_id);

                        // Clone server for this connection
                        let connection_server = server.clone();

                        // Handle this connection in a separate task
                        tokio::spawn(async move {
                            let mut message_count = 0u64;
                            loop {
                                match transport.receive().await {
                                    Ok(message) => {
                                        message_count += 1;
                                        debug!(
                                            "Connection #{} received message {}: {} bytes",
                                            connection_id,
                                            message_count,
                                            message.len()
                                        );

                                        // Process the message through MCP server
                                        match connection_server.handle_message(&message).await {
                                            Ok(Some(response)) => {
                                                debug!(
                                                    "Connection #{} sending response {}: {} bytes",
                                                    connection_id,
                                                    message_count,
                                                    response.len()
                                                );
                                                if let Err(e) = transport.send(&response).await {
                                                    error!("Connection #{} failed to send response: {} - closing connection", connection_id, e);
                                                    break;
                                                }
                                            }
                                            Ok(None) => {
                                                debug!("Connection #{} no response required for message {}", connection_id, message_count);
                                            }
                                            Err(e) => {
                                                error!("Connection #{} error processing message {}: {}", connection_id, message_count, e);
                                                warn!("Connection #{} continuing message processing despite error", connection_id);
                                            }
                                        }
                                    }
                                    Err(e) => match e {
                                        Error::Connection(msg) => {
                                            info!("Connection #{} closed gracefully: {}", connection_id, msg);
                                            break;
                                        }
                                        Error::Transport(msg) => {
                                            error!("Connection #{} transport error: {} - closing connection", connection_id, msg);
                                            break;
                                        }
                                        _ => {
                                            error!("Connection #{} unexpected error: {} - closing connection", connection_id, e);
                                            break;
                                        }
                                    }
                                }
                            }

                            // Connection cleanup
                            info!("Connection #{} closed after {} messages", connection_id, message_count);
                            if let Err(e) = transport.close().await {
                                warn!("Error closing connection #{}: {}", connection_id, e);
                            }
                        });
                    }
                    Some(Err(e)) => {
                        error!("Error accepting WebSocket connection: {}", e);
                        // Continue accepting other connections
                    }
                    None => {
                        info!("WebSocket server ended - no more connections");
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C signal - initiating graceful shutdown");
                break;
            }
        }
    }

    // Graceful shutdown
    let uptime = server_start.elapsed();
    info!(
        "Shutting down WebSocket MCP server (handled {} connections, uptime: {:?})",
        active_connections, uptime
    );

    Ok(())
}

/// Mask sensitive parts of database path for logging
fn mask_database_path(url: &str) -> String {
    if url.starts_with("sqlite:") {
        if let Some(path) = url.strip_prefix("sqlite:") {
            if let Some(file_name) = std::path::Path::new(path).file_name() {
                return format!("sqlite:.../{}", file_name.to_string_lossy());
            }
        }
        "sqlite:...".to_string()
    } else if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        // Mask PostgreSQL connection strings
        if let Ok(parsed) = ::url::Url::parse(url) {
            format!(
                "{}://***@{}/{}",
                parsed.scheme(),
                parsed.host_str().unwrap_or("***"),
                parsed.path().trim_start_matches('/')
            )
        } else {
            "database".to_string()
        }
    } else {
        "database".to_string()
    }
}
