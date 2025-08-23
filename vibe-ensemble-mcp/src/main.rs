//! Vibe Ensemble MCP Server binary
//!
//! This binary provides the main entry point for the MCP server, handling
//! database connections, service initialization, and server startup.

use std::env;
use tracing::{error, info, warn};
use tracing_subscriber::fmt::init;
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_storage::manager::DatabaseConfig;
use vibe_ensemble_storage::StorageManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    init();

    info!("Starting Vibe Ensemble MCP Server v0.1.0");

    // Get database URL from environment variable
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        warn!("DATABASE_URL not set, using default SQLite database");
        "sqlite:./vibe-ensemble.db".to_string()
    });

    info!("Connecting to database: {}", database_url);

    // Create database configuration
    let config = DatabaseConfig {
        url: database_url,
        max_connections: Some(10),
        migrate_on_startup: true,
        performance_config: None,
    };

    // Initialize storage manager
    let storage_manager = match StorageManager::new(&config).await {
        Ok(manager) => manager,
        Err(e) => {
            error!("Failed to initialize storage manager: {}", e);
            return Err(e.into());
        }
    };

    info!("Database connection established");
    info!("Database migrations completed");

    // Get services from storage manager
    let agent_service = storage_manager.agent_service();
    let issue_service = storage_manager.issue_service();
    let message_service = storage_manager.message_service();
    let knowledge_service = storage_manager.knowledge_service();

    info!("Services initialized");

    // Create MCP server with all services
    let _server = McpServer::new_with_capabilities_and_all_services(
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
    info!("Server is ready to accept connections");

    // For now, just keep the server running
    // In a full implementation, this would start an actual transport layer (WebSocket, etc.)
    tokio::signal::ctrl_c().await?;
    info!("Shutting down MCP server");

    Ok(())
}
