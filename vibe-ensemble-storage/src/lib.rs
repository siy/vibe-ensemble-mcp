//! Persistence layer for Vibe Ensemble MCP server
//!
//! This crate provides database storage and repository implementations
//! for all domain entities in the Vibe Ensemble system.

pub mod concurrent_processing;
pub mod error;
pub mod manager;
pub mod message_optimization;
pub mod migrations;
pub mod network_optimization;
pub mod performance;
pub mod repositories;
pub mod services;

pub use concurrent_processing::{
    ConcurrentProcessingConfig, ConcurrentProcessingEngine, ConcurrentProcessingStats,
    LoadBalancer, LoadBalancingRecommendation, WorkItem, WorkPriority, WorkerStats,
};
pub use error::{Error, Result};
pub use manager::StorageManager;
pub use message_optimization::{
    MessageBatch, MessageOptimizationConfig, MessageOptimizationManager, MessagePriority,
    MessageQueue, MessageQueueStats, PrioritizedMessage,
};
pub use network_optimization::{
    ConnectionPool, HttpCompression, NetworkOptimizationConfig, NetworkOptimizationManager,
    NetworkOptimizationStats, WebSocketOptimizer,
};
pub use performance::{
    CacheKey, CacheManager, CacheStats, ConnectionPoolManager, PerformanceConfig, PerformanceLayer,
    PerformanceReport, QueryOptimizer,
};
pub use services::{
    AgentPool, AgentPoolConfig, AgentPoolPerformance, AgentPoolStatistics, AgentPoolStatus,
    AgentService, AgentSession, AgentStatistics, CapabilityStats, HealthCheckResult,
    LoadBalancerRecommendation, SystemHealth, TaskAssignment,
};

/// Re-export core types for convenience
pub use vibe_ensemble_core as core;
