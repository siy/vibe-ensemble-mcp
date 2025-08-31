//! Vibe Ensemble MCP Server - Claude Code Companion
//!
//! Simplified MCP server for coordinating multiple Claude Code instances.
//! Features stdio-only transport, SQLite database, and local web dashboard.

use clap::Parser;
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vibe_ensemble_mcp::{
    server::{CoordinationServices, McpServer},
    transport::TransportFactory,
    Error,
};

/// Simplified CLI for Vibe Ensemble MCP Server
#[derive(Parser)]
#[command(name = "vibe-ensemble")]
#[command(about = "MCP server for coordinating multiple Claude Code instances")]
#[command(version)]
struct Cli {
    /// Override database path (default: ~/.vibe-ensemble/data.db)
    #[arg(long)]
    database: Option<String>,

    /// Override web host (default: 127.0.0.1)
    #[arg(long)]
    web_host: Option<String>,

    /// Override web port (default: 8080)
    #[arg(long)]
    web_port: Option<u16>,

    /// Disable database migrations on startup
    #[arg(long)]
    no_migrate: bool,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Maximum database connections (default: 10)
    #[arg(long)]
    max_connections: Option<u32>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_filter = if cli.debug {
        "vibe_ensemble_mcp=debug,vibe_ensemble_web=debug,vibe_ensemble_storage=debug"
    } else {
        "vibe_ensemble_mcp=info,vibe_ensemble_web=info,vibe_ensemble_storage=info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Vibe Ensemble MCP Server - Claude Code Companion");

    // Determine database path with smart defaults
    let database_url = cli
        .database
        .or_else(|| env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| {
            // Default to ~/.vibe-ensemble/data.db
            let home_dir = dirs::home_dir().expect("Could not determine home directory");
            let vibe_dir = home_dir.join(".vibe-ensemble");
            std::fs::create_dir_all(&vibe_dir)
                .expect("Could not create ~/.vibe-ensemble directory");
            format!("sqlite:{}", vibe_dir.join("data.db").display())
        });

    info!("Using database: {}", mask_database_path(&database_url));

    // Create database configuration with smart defaults
    let db_config = vibe_ensemble_storage::manager::DatabaseConfig {
        url: database_url,
        max_connections: Some(cli.max_connections.unwrap_or(10)),
        migrate_on_startup: !cli.no_migrate,
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

    // Run migrations if enabled
    if db_config.migrate_on_startup {
        info!("Running database migrations...");
        storage_manager.migrate().await.map_err(|e| {
            error!("Failed to run database migrations: {}", e);
            e
        })?;
        info!("Database migrations completed");
    }

    // Start embedded web dashboard
    let web_host = cli.web_host.unwrap_or_else(|| "127.0.0.1".to_string());
    let web_port = cli.web_port.unwrap_or(8080);

    info!(
        "Starting embedded web dashboard on http://{}:{}",
        web_host, web_port
    );

    let web_config = vibe_ensemble_web::server::WebConfig {
        enabled: true,
        host: web_host,
        port: web_port,
    };

    let web_server =
        vibe_ensemble_web::server::WebServer::new(web_config, storage_manager.clone()).await?;

    // Start web server in background
    let web_handle = {
        let web_server = web_server;
        tokio::spawn(async move {
            if let Err(e) = web_server.run().await {
                error!("Web server error: {}", e);
            }
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
    let server = McpServer::with_coordination(coordination_services);

    info!("MCP server initialized successfully");

    // Create stdio transport
    let mut transport = TransportFactory::stdio();
    info!("MCP server ready to accept connections via stdio");

    // Note: Real-time WebSocket updates removed - dashboard uses request/response pattern

    info!("Starting MCP stdio transport loop");

    // Main server loop
    let mut loop_count = 0u64;
    loop {
        loop_count += 1;

        // Log progress periodically for monitoring
        if loop_count % 100 == 0 {
            debug!(
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
                        warn!("Continuing message processing despite error");
                    }
                }
            }
            Err(e) => match e {
                Error::Connection(msg) => {
                    info!("Connection closed gracefully: {}", msg);
                    break;
                }
                Error::Transport(msg) => {
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

    // Shutdown
    info!(
        "Shutting down MCP server after {} loop iterations",
        loop_count
    );
    transport.close().await?;

    // Cancel web server
    web_handle.abort();

    info!("Vibe Ensemble MCP Server shut down gracefully");
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
        "sqlite:..."
    } else {
        "database"
    }
    .to_string()
}
