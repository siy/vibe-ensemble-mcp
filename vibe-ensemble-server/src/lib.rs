//! Main server application for Vibe Ensemble MCP
//!
//! This crate provides the main server application that orchestrates
//! all components of the Vibe Ensemble system.

use clap::ValueEnum;

pub mod config;
pub mod error;
pub mod server;

#[cfg(test)]
mod config_tests;

pub use error::{Error, Result};

/// Re-export all core modules for convenience
pub use vibe_ensemble_core as core;
pub use vibe_ensemble_mcp as mcp;
pub use vibe_ensemble_prompts as prompts;
pub use vibe_ensemble_storage as storage;
pub use vibe_ensemble_web as web;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum McpTransport {
    /// Use stdio transport (for Claude Code integration)
    Stdio,
    /// Use WebSocket transport
    Websocket,
    /// Use Server-Sent Events transport (HTTP streaming)
    Sse,
    /// Support both stdio and websocket transports
    Both,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OperationMode {
    /// Full server with API, Web Dashboard, and MCP endpoints (default)
    Full,
    /// MCP server only with stdio transport
    McpOnly,
    /// Web dashboard only
    WebOnly,
    /// API server only
    ApiOnly,
}
