//! Vibe Ensemble Unified Server
//!
//! Single binary that supports multiple operational modes:
//! - Full mode (default): HTTP API + Web Dashboard + MCP endpoints
//! - MCP-only mode: Just MCP server with stdio transport for Claude Code integration
//! - Web-only mode: Just web dashboard
//! - API-only mode: Just HTTP API without web interface

use clap::Parser;
use std::env;
use tracing::{error, info, warn};
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
    #[arg(long, value_enum, default_value = "websocket")]
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

    // Handle MCP-only mode with stdio transport
    if matches!(operation_mode, OperationMode::McpOnly) && matches!(cli.transport, McpTransport::Stdio) {
        return run_mcp_stdio_mode().await;
    }

    info!("Starting Vibe Ensemble Server in {:?} mode", operation_mode);

    // Load base configuration
    let mut config = if let Some(ref config_path) = cli.config {
        Config::load_from_file(&config_path).map_err(|e| {
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

    // Create and start server
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
            config.web.enabled = false;
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

/// Run MCP server in stdio mode (for Claude Code integration)
async fn run_mcp_stdio_mode() -> Result<()> {
    info!("Starting Vibe Ensemble MCP Server in stdio mode");

    // Get database URL from environment variable
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        warn!("DATABASE_URL not set, using default SQLite database");
        "sqlite:./vibe_ensemble.db".to_string()
    });

    info!("Connecting to database: {}", database_url);

    // Create database configuration
    let config = vibe_ensemble_storage::manager::DatabaseConfig {
        url: database_url,
        max_connections: Some(10),
        migrate_on_startup: true,
        performance_config: None,
    };

    // Initialize storage manager
    let storage_manager = vibe_ensemble_storage::StorageManager::new(&config)
        .await
        .map_err(|e| {
            error!("Failed to initialize storage manager: {}", e);
            e
        })?;

    info!("Database connection established");
    info!("Database migrations completed");

    // Get services from storage manager
    let agent_service = storage_manager.agent_service();
    let issue_service = storage_manager.issue_service();
    let message_service = storage_manager.message_service();
    let knowledge_service = storage_manager.knowledge_service();

    info!("Services initialized");

    // Create MCP server with all services
    let server = vibe_ensemble_mcp::server::McpServer::new_with_capabilities_and_all_services(
        vibe_ensemble_mcp::protocol::ServerCapabilities {
            experimental: None,
            logging: None,
            prompts: Some(vibe_ensemble_mcp::protocol::PromptsCapability {
                list_changed: Some(true),
            }),
            resources: Some(vibe_ensemble_mcp::protocol::ResourcesCapability {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            tools: Some(vibe_ensemble_mcp::protocol::ToolsCapability {
                list_changed: Some(true),
            }),
            vibe_agent_management: Some(true),
            vibe_issue_tracking: Some(true),
            vibe_messaging: Some(true),
            vibe_knowledge_management: Some(true),
        },
        agent_service,
        issue_service,
        message_service,
        knowledge_service,
    );

    info!("MCP server initialized successfully");

    // Create stdio transport
    let mut transport = vibe_ensemble_mcp::transport::TransportFactory::stdio();
    info!("Server is ready to accept connections via stdio");

    // Main server loop - handle messages from stdin and send responses to stdout
    loop {
        match transport.receive().await {
            Ok(message) => {
                tracing::debug!("Received message: {}", message);
                
                // Process the message
                match server.handle_message(&message).await {
                    Ok(Some(response)) => {
                        tracing::debug!("Sending response: {}", response);
                        if let Err(e) = transport.send(&response).await {
                            error!("Failed to send response: {}", e);
                            break;
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("No response required");
                    }
                    Err(e) => {
                        error!("Error processing message: {}", e);
                        // Continue processing other messages
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Transport error: {}", e);
                break;
            }
        }
    }

    info!("Shutting down MCP server");
    transport.close().await?;

    Ok(())
}
