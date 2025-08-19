//! Security and authentication layer for Vibe Ensemble MCP server
//!
//! This module provides comprehensive security features including:
//! - JWT token-based authentication
//! - Password hashing and verification
//! - Role-based access control (RBAC)
//! - Rate limiting
//! - Message encryption
//! - Audit logging
//! - Security middleware

pub mod audit;
pub mod auth;
pub mod crypto;
pub mod error;
pub mod jwt;
pub mod middleware;
pub mod models;
pub mod permissions;
pub mod rate_limiting;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use audit::*;
pub use auth::*;
pub use crypto::*;
pub use error::*;
pub use jwt::*;
pub use middleware::*;
pub use models::*;
pub use permissions::*;
pub use rate_limiting::*;
