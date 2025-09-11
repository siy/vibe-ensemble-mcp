use anyhow::Result;
use clap::Parser;
use tracing::info;
use vibe_ensemble_mcp::{config::Config, server::run_server};

#[derive(Parser)]
#[command(name = "vibe-ensemble-mcp")]
#[command(about = "A multi-agent coordination MCP server")]
struct Args {
    /// Database file path
    #[arg(long, default_value = "./vibe-ensemble.db")]
    database_path: String,

    /// Server host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(long, default_value = "3000")]
    port: u16,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&args.log_level)
        .init();

    info!("Starting Vibe-Ensemble MCP Server");
    info!("Database: {}", args.database_path);
    info!("Server: {}:{}", args.host, args.port);

    let config = Config {
        database_path: args.database_path,
        host: args.host,
        port: args.port,
    };

    run_server(config).await?;

    Ok(())
}
