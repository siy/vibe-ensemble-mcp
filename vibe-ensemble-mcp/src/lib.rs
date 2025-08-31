//! MCP protocol implementation for Vibe Ensemble
//!
//! This crate provides the Model Context Protocol (MCP) implementation
//! for communication between the Vibe Ensemble server and Claude Code instances.
//!
//! # Architecture
//!
//! The MCP implementation follows the official MCP 2024-11-05 specification
//! and includes the following components:
//!
//! - **[`protocol`]**: JSON-RPC 2.0 message types and MCP protocol definitions
//! - **[`server`]**: MCP server implementation with capability negotiation
//! - **[`client`]**: MCP client for connecting to MCP servers
//! - **[`transport`]**: Transport layer supporting WebSocket and in-memory connections
//! - **[`error`]**: Comprehensive error handling for protocol operations
//!
//! # Protocol Flow
//!
//! ```text
//! Client                          Server
//!   |                               |
//!   |-- initialize ---------------->|
//!   |<------------- initialize -----|
//!   |                               |
//!   |-- ping ---------------------->|
//!   |<------------------ pong ------|
//!   |                               |
//!   |-- tools/list ---------------->|
//!   |<-------------- tools list ----|
//!   |                               |
//!   |-- vibe/agent/register ------->|
//!   |<---------- registration ------|
//! ```
//!
//! # Example Usage
//!
//! ## Server
//!
//! ```rust
//! use vibe_ensemble_mcp::{server::McpServer, protocol::*};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = McpServer::new();
//!
//! // Handle an initialization request
//! let request = r#"{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}"#;
//! let response = server.handle_message(request).await?;
//! println!("Response: {:?}", response);
//! # Ok(())
//! # }
//! ```
//!
//! ## Client
//!
//! ```rust
//! use vibe_ensemble_mcp::{client::McpClient, protocol::*, transport::TransportFactory};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let (client_transport, _server_transport) = TransportFactory::in_memory_pair();
//!
//! let client_info = ClientInfo {
//!     name: "example-client".to_string(),
//!     version: "1.0.0".to_string(),
//! };
//! let capabilities = ClientCapabilities {
//!     experimental: None,
//!     sampling: None,
//! };
//!
//! let mut client = McpClient::new(client_transport, client_info, capabilities);
//! // Note: In real usage, you would initialize with a connected server
//! // let result = client.initialize().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Vibe Ensemble Extensions
//!
//! This implementation includes Vibe Ensemble-specific extensions:
//!
//! - **Agent Registration**: `vibe/agent/register` for Claude Code agents
//! - **Issue Tracking**: `vibe/issue/*` methods for task management
//! - **Knowledge Management**: `vibe/knowledge/*` methods for pattern storage
//! - **Real-time Messaging**: `vibe/message/*` methods for agent communication
//!
//! # Transport Options
//!
//! - **WebSocket**: Real-time bidirectional communication for production
//! - **In-Memory**: Fast local communication for testing and development

pub mod client;
pub mod error;
pub mod protocol;
pub mod server;
pub mod transport;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod communication_tests;

#[cfg(test)]
mod coordination_tests;

// NOTE: Transport compliance tests temporarily disabled due to architecture simplification (Phase 1)
// These tests reference removed SSE and WebSocket transports and need to be updated
// #[cfg(test)]
// mod transport_compliance_tests;

// NOTE: Claude Code integration tests temporarily disabled due to architecture simplification (Phase 1)
// These tests reference removed transport implementations and need to be updated  
// #[cfg(any(test, feature = "test-support"))]
// pub mod claude_code_integration_tests;

pub use error::{Error, Result};

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
