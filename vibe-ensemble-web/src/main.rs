//! Web dashboard binary for Vibe Ensemble MCP Server

use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vibe_ensemble_storage::{manager::DatabaseConfig, StorageManager};
use vibe_ensemble_web::{server::WebConfig, WebServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,vibe_ensemble_web=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Vibe Ensemble Web Dashboard");

    // Initialize storage manager
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./vibe-ensemble.db".to_string());

    let db_config = DatabaseConfig {
        url: database_url,
        max_connections: None,
        migrate_on_startup: true,
        performance_config: None,
    };
    let storage = Arc::new(
        StorageManager::new(&db_config)
            .await
            .expect("Failed to initialize storage"),
    );

    // Run database migrations
    storage
        .migrate()
        .await
        .expect("Failed to run database migrations");

    // Create web server configuration
    let config = WebConfig {
        enabled: true,
        host: std::env::var("WEB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("WEB_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .expect("Invalid WEB_PORT"),
    };

    // Create and run web server
    let web_server = WebServer::new(config, storage).await?;
    web_server.run().await?;

    Ok(())
}