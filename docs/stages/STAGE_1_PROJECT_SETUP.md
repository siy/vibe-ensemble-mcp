# Stage 1: Project Setup

**Duration**: 1-2 hours  
**Goal**: Basic Rust project structure with HTTP server

## Overview

This stage establishes the foundational Rust project structure with a basic HTTP server, logging framework, and CLI argument parsing. The server will have a health check endpoint and be ready for MCP protocol implementation in subsequent stages.

## Objectives

1. Initialize Cargo project with proper dependencies
2. Set up basic HTTP server using Axum
3. Implement logging and configuration framework
4. Add CLI argument parsing for database configuration
5. Create project directory structure
6. Add health check endpoint for server validation

## Dependencies

Add the following to `Cargo.toml`:

```toml
[package]
name = "vibe-ensemble-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
# Async runtime
tokio = { version = "1.0", features = ["full"] }

# HTTP server
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Database (for future stages)
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono", "uuid"] }

# Utilities
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"

# CLI and config
clap = { version = "4.0", features = ["derive"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Project Structure

Create the following directory structure:

```
src/
├── main.rs              # Entry point and CLI
├── lib.rs              # Library root
├── config.rs           # Configuration management
├── error.rs           # Error types and handling
├── server.rs          # HTTP server setup
├── database/          # Database operations (future)
│   └── mod.rs
├── mcp/               # MCP protocol implementation (future)
│   └── mod.rs
├── workers/           # Worker management (future)
│   └── mod.rs
└── ticket/            # Ticket system (future)
    └── mod.rs
```

## Implementation Details

### 1. Main Entry Point (`src/main.rs`)

```rust
use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
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
```

### 2. Configuration (`src/config.rs`)

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn database_url(&self) -> String {
        format!("sqlite:{}", self.database_path)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
```

### 3. Error Handling (`src/error.rs`)

```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(ref err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            AppError::Json(ref err) => (StatusCode::BAD_REQUEST, err.to_string()),
            AppError::BadRequest(ref message) => (StatusCode::BAD_REQUEST, message.clone()),
            AppError::NotFound(ref message) => (StatusCode::NOT_FOUND, message.clone()),
            AppError::Internal(ref err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
        };

        let body = json!({
            "error": error_message
        });

        (status, axum::Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
```

### 4. HTTP Server (`src/server.rs`)

```rust
use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, error};

use crate::{config::Config, error::Result};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    // Database connection will be added in Stage 2
}

pub async fn run_server(config: Config) -> Result<()> {
    let state = AppState {
        config: config.clone(),
    };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(mcp_handler)) // Future MCP endpoint
        .layer(cors)
        .with_state(state);

    let address = config.server_address();
    info!("Server listening on {}", address);

    let listener = tokio::net::TcpListener::bind(&address).await?;
    
    match axum::serve(listener, app).await {
        Ok(_) => info!("Server stopped gracefully"),
        Err(e) => error!("Server error: {}", e),
    }

    Ok(())
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "vibe-ensemble-mcp",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn mcp_handler(
    State(_state): State<AppState>,
    _payload: Json<Value>,
) -> Result<Json<Value>> {
    // Placeholder for MCP protocol implementation
    Ok(Json(json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32601,
            "message": "Method not implemented yet"
        }
    })))
}
```

### 5. Library Root (`src/lib.rs`)

```rust
pub mod config;
pub mod error;
pub mod server;

// Future modules
pub mod database {
    // Will be implemented in Stage 2
}

pub mod mcp {
    // Will be implemented in Stage 3
}

pub mod workers {
    // Will be implemented in Stage 4
}

pub mod tickets {
    // Will be implemented in Stage 5
}
```

## Testing

### 1. Compilation Test
```bash
cargo check
cargo build
```

### 2. Health Check Test
```bash
# Start server
cargo run

# In another terminal
curl http://localhost:3000/health
```

Expected response:
```json
{
  "status": "healthy",
  "service": "vibe-ensemble-mcp",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 3. MCP Endpoint Test
```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"test","id":1}'
```

Expected response:
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32601,
    "message": "Method not implemented yet"
  }
}
```

## Validation Checklist

- [ ] Project compiles without warnings
- [ ] Server starts on specified host/port
- [ ] Health check endpoint returns proper JSON
- [ ] MCP endpoint accepts POST requests
- [ ] CLI arguments work correctly
- [ ] Logging outputs to console
- [ ] CORS headers are set properly
- [ ] Graceful shutdown works (Ctrl+C)

## Troubleshooting

### Common Issues

1. **Port already in use**
   ```bash
   # Find process using port 3000
   lsof -i :3000
   # Kill the process or use different port
   cargo run -- --port 3001
   ```

2. **Permission denied for database path**
   ```bash
   # Ensure directory exists and is writable
   mkdir -p $(dirname ./vibe-ensemble.db)
   touch ./vibe-ensemble.db
   ```

3. **Dependencies not compiling**
   ```bash
   # Clean and rebuild
   cargo clean
   cargo build
   ```

## Next Steps

After completing Stage 1:
1. Verify all tests pass
2. Update progress in [TODO.md](../TODO.md)
3. Proceed to [Stage 2: Database Layer](STAGE_2_DATABASE_LAYER.md)

## Files Created

- `Cargo.toml` - Project configuration and dependencies
- `src/main.rs` - CLI entry point
- `src/lib.rs` - Library root with module structure
- `src/config.rs` - Configuration management
- `src/error.rs` - Error handling types
- `src/server.rs` - HTTP server implementation
- Module placeholders for future stages

## Success Criteria

✅ HTTP server runs without errors  
✅ Health check endpoint responds correctly  
✅ CLI arguments are parsed and applied  
✅ Logging is properly configured  
✅ Project structure is ready for next stages