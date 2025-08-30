//! Vibe Ensemble Unified Server
//!
//! Single binary that supports multiple operational modes:
//! - Full mode (default): HTTP API + Web Dashboard + MCP endpoints
//! - MCP-only mode: Just MCP server with stdio transport for Claude Code integration
//! - Web-only mode: Just web dashboard
//! - API-only mode: Just HTTP API without web interface

use clap::Parser;
use std::env;
use tracing::{debug, error, info, warn};

/// Helper function to get database scheme type for safe logging
fn db_scheme(url: &str) -> &'static str {
    if url.starts_with("postgres://") {
        "PostgreSQL"
    } else if url.starts_with("mysql://") {
        "MySQL"
    } else if url.starts_with("sqlite:") {
        "SQLite"
    } else {
        "Unknown DB"
    }
}
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vibe_ensemble_server::{config::Config, server::Server, McpTransport, OperationMode, Result};

/// Unified Vibe Ensemble Server
#[derive(Parser)]
#[command(name = "vibe-ensemble")]
#[command(about = "Unified Vibe Ensemble Server supporting multiple operational modes")]
#[command(version)]
struct Cli {
    /// Operation mode
    #[arg(long, value_enum, default_value = "full")]
    mode: OperationMode,

    /// MCP-only mode (equivalent to --mode=mcp-only)
    #[arg(long, conflicts_with = "mode")]
    mcp_only: bool,

    /// Web-only mode (equivalent to --mode=web-only)
    #[arg(long, conflicts_with = "mode")]
    web_only: bool,

    /// API-only mode (equivalent to --mode=api-only)
    #[arg(long, conflicts_with = "mode")]
    api_only: bool,

    /// MCP transport type (when MCP is enabled)
    #[arg(long, value_enum, default_value = "both")]
    transport: McpTransport,

    /// Override server host
    #[arg(long)]
    host: Option<String>,

    /// Override server port
    #[arg(long)]
    port: Option<u16>,

    /// Override web host
    #[arg(long)]
    web_host: Option<String>,

    /// Override web port
    #[arg(long)]
    web_port: Option<u16>,

    /// Path to configuration file
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine final operation mode
    let operation_mode = if cli.mcp_only {
        OperationMode::McpOnly
    } else if cli.web_only {
        OperationMode::WebOnly
    } else if cli.api_only {
        OperationMode::ApiOnly
    } else {
        cli.mode
    };

    // Initialize tracing based on mode
    let log_filter = match operation_mode {
        OperationMode::McpOnly => "vibe_ensemble_mcp=debug,vibe_ensemble_server=info",
        OperationMode::WebOnly => "vibe_ensemble_web=debug,vibe_ensemble_server=info",
        _ => "vibe_ensemble_server=debug,tower_http=debug",
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Vibe Ensemble Server in {:?} mode", operation_mode);

    // Load base configuration
    let mut config = if let Some(ref config_path) = cli.config {
        Config::load_from_file(config_path).map_err(|e| {
            error!("Failed to load configuration from {}: {}", config_path, e);
            e
        })?
    } else {
        Config::load().map_err(|e| {
            error!("Failed to load configuration: {}", e);
            e
        })?
    };

    // Apply CLI overrides to configuration
    apply_cli_overrides(&mut config, &cli, operation_mode);

    info!("Configuration loaded successfully");

    // Handle MCP-only mode with stdio transport (use specialized execution only if web is disabled)
    if matches!(operation_mode, OperationMode::McpOnly)
        && matches!(cli.transport, McpTransport::Stdio)
        && !config.web.enabled
    {
        return run_mcp_stdio_mode_unified(&config).await;
    }

    // Create and start server for all other modes
    let server = Server::new(config, operation_mode, cli.transport).await?;

    info!("Server initialized, starting...");

    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        return Err(e);
    }

    info!("Vibe Ensemble Server shut down gracefully");
    Ok(())
}

/// Apply CLI argument overrides to configuration
fn apply_cli_overrides(config: &mut Config, cli: &Cli, mode: OperationMode) {
    // Override server settings
    if let Some(host) = &cli.host {
        config.server.host = host.clone();
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }

    // Override web settings
    if let Some(web_host) = &cli.web_host {
        config.web.host = web_host.clone();
    }
    if let Some(web_port) = cli.web_port {
        config.web.port = web_port;
    }

    // Configure components based on operation mode
    match mode {
        OperationMode::Full => {
            // All components enabled (default config)
        }
        OperationMode::McpOnly => {
            // Keep web dashboard enabled for monitoring even in MCP-only mode
            // This allows users to monitor the system while using Claude Code integration
            // MCP will be handled separately
        }
        OperationMode::WebOnly => {
            // Only web enabled, API server disabled by running web server standalone
        }
        OperationMode::ApiOnly => {
            config.web.enabled = false;
        }
    }
}

/// Run MCP server in stdio mode using unified configuration (for Claude Code integration)
async fn run_mcp_stdio_mode_unified(config: &Config) -> Result<()> {
    info!("Starting Vibe Ensemble MCP Server in stdio mode");

    // Use unified config database URL with environment variable override support
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        info!(
            "DATABASE_URL not set, using configuration default for {}",
            db_scheme(&config.database.url)
        );
        config.database.url.clone()
    });

    info!("Connecting to database ({})", db_scheme(&database_url));

    // Create database configuration using unified config values
    let db_config = vibe_ensemble_storage::manager::DatabaseConfig {
        url: database_url,
        max_connections: config.database.max_connections,
        migrate_on_startup: config.database.migrate_on_startup,
        performance_config: None,
    };

    // Initialize storage manager
    let storage_manager = vibe_ensemble_storage::StorageManager::new(&db_config)
        .await
        .map_err(|e| {
            error!("Failed to initialize storage manager: {}", e);
            e
        })?;

    info!("Database connection established");

    if config.database.migrate_on_startup {
        storage_manager.migrate().await.map_err(|e| {
            error!("Failed to run database migrations: {}", e);
            e
        })?;
        info!("Database migrations completed");
    } else {
        info!("Database auto-migration disabled");
    }

    // Get services from storage manager
    let agent_service = storage_manager.agent_service();
    let issue_service = storage_manager.issue_service();
    let message_service = storage_manager.message_service();
    let knowledge_service = storage_manager.knowledge_service();

    // Create coordination service manually from repositories
    let coordination_service =
        std::sync::Arc::new(vibe_ensemble_storage::services::CoordinationService::new(
            storage_manager.agents(),
            storage_manager.issues(),
            storage_manager.messages(),
        ));

    info!("Services initialized");

    // Create coordination services bundle
    let coordination_services = vibe_ensemble_mcp::server::CoordinationServices::new(
        agent_service,
        issue_service,
        message_service,
        coordination_service,
        knowledge_service,
    );

    // Create MCP server with coordination services
    let server = vibe_ensemble_mcp::server::McpServer::with_coordination(coordination_services);

    info!("MCP server initialized successfully");

    // Create stdio transport with enhanced features
    let mut transport = vibe_ensemble_mcp::transport::TransportFactory::stdio();
    info!("Server is ready to accept connections via stdio");

    info!("Starting enhanced MCP stdio transport loop");

    // Enhanced main server loop with connection state management and improved error handling
    let mut loop_count = 0u64;
    loop {
        loop_count += 1;

        // Log progress periodically for monitoring
        if loop_count % 100 == 0 {
            info!(
                "Processing loop iteration {} - connection active",
                loop_count
            );
        }

        match transport.receive().await {
            Ok(message) => {
                debug!(
                    "Received message (loop {}): {} bytes",
                    loop_count,
                    message.len()
                );

                // Process the message through MCP server
                match server.handle_message(&message).await {
                    Ok(Some(response)) => {
                        debug!(
                            "Sending response (loop {}): {} bytes",
                            loop_count,
                            response.len()
                        );
                        if let Err(e) = transport.send(&response).await {
                            error!("Failed to send response: {} - closing connection", e);
                            break;
                        }
                    }
                    Ok(None) => {
                        debug!("No response required for message (loop {})", loop_count);
                    }
                    Err(e) => {
                        error!("Error processing message (loop {}): {}", loop_count, e);

                        // For critical MCP protocol errors, we might want to send error response
                        // but continue processing other messages for resilience
                        warn!("Continuing message processing despite error");
                    }
                }
            }
            Err(e) => match e {
                vibe_ensemble_mcp::Error::Connection(msg) => {
                    info!("Connection closed gracefully: {}", msg);
                    break;
                }
                vibe_ensemble_mcp::Error::Transport(msg) => {
                    error!("Transport error: {} - closing connection", msg);
                    break;
                }
                _ => {
                    error!(
                        "Unexpected error in transport loop: {} - closing connection",
                        e
                    );
                    break;
                }
            },
        }
    }

    // Log shutdown
    info!(
        "Shutting down MCP server after {} loop iterations",
        loop_count
    );

    transport.close().await?;

    Ok(())
}
