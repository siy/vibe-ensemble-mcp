use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use vibe_ensemble_mcp::{
    config::Config, configure::configure_claude_code, permissions::PermissionMode,
    server::run_server,
};

#[derive(Parser)]
#[command(name = "vibe-ensemble-mcp")]
#[command(about = "A multi-agent coordination MCP server")]
struct Args {
    /// Configure Claude Code integration (generates .mcp.json and .claude/ files)
    #[arg(long)]
    configure_claude_code: bool,

    /// Database file path
    #[arg(long, default_value = "./.vibe-ensemble-mcp/vibe-ensemble.db")]
    database_path: String,

    /// Server host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(long, default_value = "3276")]
    port: u16,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Disable automatic respawning of workers on startup for unfinished tasks
    #[arg(long)]
    no_respawn: bool,

    /// Permission mode for worker processes
    #[arg(long, default_value_t = PermissionMode::File)]
    permission_mode: PermissionMode,

    /// Timeout for client tool calls in seconds
    #[arg(long, default_value = "30")]
    client_tool_timeout_secs: u64,

    /// Maximum concurrent client requests
    #[arg(long, default_value = "50")]
    max_concurrent_client_requests: usize,

    /// Update check interval in hours
    #[arg(long, default_value = "4")]
    update_check_interval_hours: u64,

    /// Disable automatic update checks
    #[arg(long)]
    disable_update_checks: bool,

    /// Upgrade to the latest version
    #[arg(long)]
    upgrade: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle upgrade mode
    if args.upgrade {
        return handle_upgrade();
    }

    // Handle configuration mode
    if args.configure_claude_code {
        configure_claude_code(&args.host, args.port, args.permission_mode).await?;
        return Ok(());
    }

    // Initialize tracing with both console and file logging
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    // Create logs directory
    let logs_dir = std::path::Path::new(".vibe-ensemble-mcp/logs");
    std::fs::create_dir_all(logs_dir)?;

    let file_appender = tracing_appender::rolling::daily(logs_dir, "server.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Guard is kept alive by the variable scope and will be properly cleaned up on exit

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter.clone()))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_filter(env_filter),
        )
        .init();

    info!("Starting Vibe-Ensemble MCP Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Database: {}", args.database_path);
    info!("Server: {}:{}", args.host, args.port);
    info!("Permission mode: {}", args.permission_mode.as_str());
    info!("Respawn disabled: {}", args.no_respawn);

    let config = Config {
        database_path: args.database_path,
        host: args.host,
        port: args.port,
        no_respawn: args.no_respawn,
        permission_mode: args.permission_mode,
        client_tool_timeout_secs: args.client_tool_timeout_secs,
        max_concurrent_client_requests: args.max_concurrent_client_requests,
        update_check_interval_hours: args.update_check_interval_hours,
        disable_update_checks: args.disable_update_checks,
    };

    run_server(config).await?;

    Ok(())
}

fn handle_upgrade() -> Result<()> {
    println!("Starting upgrade process...");

    let os = std::env::consts::OS;
    let result = match os {
        "macos" | "linux" => {
            // Use sh to execute the curl command
            std::process::Command::new("sh")
                .arg("-c")
                .arg("curl -fsSL https://vibeensemble.dev/install.sh | sh")
                .status()
        }
        "windows" => {
            // Use PowerShell to execute the download and install
            std::process::Command::new("powershell")
                .arg("-Command")
                .arg("iwr -useb https://vibeensemble.dev/install.ps1 | iex")
                .status()
        }
        _ => {
            eprintln!("Unsupported operating system: {}", os);
            eprintln!("Please manually download the latest release from:");
            eprintln!("https://github.com/siy/vibe-ensemble-mcp/releases");
            std::process::exit(1);
        }
    };

    match result {
        Ok(status) if status.success() => {
            println!("\n✓ Upgrade completed successfully!");
            println!("Please restart the server to use the new version.");
            Ok(())
        }
        Ok(status) => {
            eprintln!("\n✗ Upgrade failed with exit code: {:?}", status.code());
            eprintln!("Please try manually downloading from:");
            eprintln!("https://github.com/siy/vibe-ensemble-mcp/releases");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("\n✗ Failed to execute upgrade command: {}", e);
            eprintln!("Please try manually downloading from:");
            eprintln!("https://github.com/siy/vibe-ensemble-mcp/releases");
            std::process::exit(1);
        }
    }
}
