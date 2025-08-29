//! Coordinator-Worker Integration Testing Framework
//!
//! This module provides a comprehensive testing framework for validating
//! coordinator-worker workflows using real MCP tool interactions, git
//! worktree isolation, and file system verification.

pub mod framework;
pub mod mock_agents;
pub mod worktree_manager;
pub mod file_system_verifier;

// Re-export main types for convenience
pub use framework::{TestContext, TestDataBuilder, PerformanceMonitor, TestAssertions};
pub use mock_agents::{MockCoordinator, MockWorker, MockAgentFactory, AgentInteractionSimulator};
pub use worktree_manager::{GitWorktreeManager, GitWorktreeStatus, WorktreeTestScenario};
pub use file_system_verifier::{
    FileSystemVerifier, VerificationRules, DirectoryStructure, SizeConstraint, 
    TimeWindow, FileVerification, BatchVerificationResult, FileVerificationError
};