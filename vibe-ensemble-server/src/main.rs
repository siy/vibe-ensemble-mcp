//! Vibe Ensemble MCP Server
//!
//! Main entry point for the Vibe Ensemble MCP server application.

use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vibe_ensemble_server::{config::Config, server::Server, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vibe_ensemble_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Vibe Ensemble MCP Server");

    // Load configuration
    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Configuration loaded successfully");

    // Create and start server
    let server = Server::new(config).await?;
    
    info!("Server initialized, starting...");
    
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        return Err(e);
    }

    info!("Vibe Ensemble MCP Server shut down gracefully");
    Ok(())
}