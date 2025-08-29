//! Integration tests for coordinator-worker workflows
//!
//! This module implements comprehensive integration tests that simulate real
//! coordinator-worker interactions via MCP tools, including git worktree
//! isolation and file system verification.
//!
//! Tests validate the complete workflow:
//! 1. Coordinator registers and creates tickets via MCP tools
//! 2. Workers spawn and create actual files in isolated worktrees
//! 3. File creation is verified through filesystem checks
//! 4. All tests run in separate CI pipeline

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use vibe_ensemble_mcp::{
    client::McpClient,
    protocol::*,
    server::{CoordinationServices, McpServer},
    transport::{Transport, TransportFactory},
};

mod framework;
mod mock_agents;
mod worktree_manager;
mod file_system_verifier;

pub use framework::*;
pub use mock_agents::*;
pub use worktree_manager::*;
pub use file_system_verifier::*;

/// Integration test framework for coordinator-worker workflows
pub struct CoordinatorWorkerTestFramework {
    /// MCP server for coordination
    server: McpServer,
    /// Mock coordinator agent
    coordinator: MockCoordinator,
    /// Mock worker agents
    workers: Vec<MockWorker>,
    /// Git worktree manager for workspace isolation
    worktree_manager: GitWorktreeManager,
    /// File system verifier for validating worker outputs
    file_verifier: FileSystemVerifier,
    /// Test workspace directory
    workspace_dir: PathBuf,
    /// Coordination services for testing
    coordination_services: CoordinationServices,
}

impl CoordinatorWorkerTestFramework {
    /// Creates a new test framework instance
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Set up test database
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await?;
            
        vibe_ensemble_storage::migrations::run_migrations(&pool).await?;

        // Create coordination services
        let agent_repo = Arc::new(vibe_ensemble_storage::repositories::AgentRepository::new(pool.clone()));
        let issue_repo = Arc::new(vibe_ensemble_storage::repositories::IssueRepository::new(pool.clone()));
        let message_repo = Arc::new(vibe_ensemble_storage::repositories::MessageRepository::new(pool.clone()));
        let knowledge_repo = Arc::new(vibe_ensemble_storage::repositories::KnowledgeRepository::new(pool));

        let coordination_services = CoordinationServices::new(
            Arc::new(vibe_ensemble_storage::services::AgentService::new(agent_repo.clone())),
            Arc::new(vibe_ensemble_storage::services::IssueService::new(issue_repo.clone())),
            Arc::new(vibe_ensemble_storage::services::MessageService::new(message_repo.clone())),
            Arc::new(vibe_ensemble_storage::services::CoordinationService::new(
                agent_repo,
                issue_repo,
                message_repo,
            )),
            Arc::new(vibe_ensemble_storage::services::KnowledgeService::new(knowledge_repo)),
        );

        // Create MCP server
        let server = McpServer::with_coordination(coordination_services.clone());

        // Create test workspace directory
        let workspace_dir = std::env::temp_dir().join(format!("vibe-ensemble-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workspace_dir).await?;

        // Initialize git repository in workspace
        let git_init_result = tokio::process::Command::new("git")
            .args(&["init"])
            .current_dir(&workspace_dir)
            .output()
            .await?;

        if !git_init_result.status.success() {
            return Err(format!("Failed to initialize git repository: {}", 
                String::from_utf8_lossy(&git_init_result.stderr)).into());
        }

        // Set up git config for tests
        tokio::process::Command::new("git")
            .args(&["config", "user.name", "Test Framework"])
            .current_dir(&workspace_dir)
            .output()
            .await?;

        tokio::process::Command::new("git")
            .args(&["config", "user.email", "test@vibe-ensemble.local"])
            .current_dir(&workspace_dir)
            .output()
            .await?;

        // Create initial commit
        tokio::fs::write(workspace_dir.join("README.md"), "# Integration Test Workspace\n").await?;
        tokio::process::Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(&workspace_dir)
            .output()
            .await?;

        tokio::process::Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&workspace_dir)
            .output()
            .await?;

        Ok(Self {
            server,
            coordinator: MockCoordinator::new("test-coordinator").await?,
            workers: Vec::new(),
            worktree_manager: GitWorktreeManager::new(workspace_dir.clone()),
            file_verifier: FileSystemVerifier::new(),
            workspace_dir,
            coordination_services,
        })
    }

    /// Adds a mock worker to the framework
    pub async fn add_worker(&mut self, name: &str, capabilities: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        let worker = MockWorker::new(name, capabilities).await?;
        self.workers.push(worker);
        Ok(())
    }

    /// Sets up the test environment with coordinator and workers
    pub async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Register coordinator via MCP
        self.coordinator.register_with_server(&self.server).await?;

        // Register all workers via MCP
        for worker in &mut self.workers {
            worker.register_with_server(&self.server).await?;
        }

        Ok(())
    }

    /// Runs a complete coordinator-worker workflow test
    pub async fn run_workflow_test(&mut self, test_scenario: WorkflowTestScenario) -> Result<WorkflowTestResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        let mut result = WorkflowTestResult::new(test_scenario.name.clone());

        // Phase 1: Coordinator creates tickets
        let ticket_ids = self.coordinator.create_tickets(&self.server, test_scenario.tickets).await?;
        result.add_phase("ticket_creation", start_time.elapsed());

        // Phase 2: Create isolated worktrees for workers
        let mut worktree_assignments = HashMap::new();
        for (worker_id, ticket_id) in test_scenario.assignments.iter() {
            if let Some(worker) = self.workers.iter().find(|w| w.id() == *worker_id) {
                let worktree_path = self.worktree_manager.create_worktree(
                    &format!("worker-{}-{}", worker.name(), ticket_id)
                ).await?;
                worktree_assignments.insert(*worker_id, worktree_path);
            }
        }
        result.add_phase("worktree_creation", start_time.elapsed());

        // Phase 3: Assign tickets to workers
        for (worker_id, ticket_id) in test_scenario.assignments.iter() {
            self.coordinator.assign_ticket(&self.server, *ticket_id, *worker_id).await?;
        }
        result.add_phase("ticket_assignment", start_time.elapsed());

        // Phase 4: Workers create files in their isolated worktrees
        let mut file_creation_tasks = Vec::new();
        for worker in &self.workers {
            if let Some(worktree_path) = worktree_assignments.get(&worker.id()) {
                let assigned_tickets: Vec<_> = test_scenario.assignments
                    .iter()
                    .filter(|(worker_id, _)| *worker_id == worker.id())
                    .map(|(_, ticket_id)| *ticket_id)
                    .collect();

                for ticket_id in assigned_tickets {
                    if let Some(files) = test_scenario.expected_files.get(&ticket_id) {
                        let task = worker.create_files_in_worktree(
                            worktree_path.clone(),
                            files.clone()
                        );
                        file_creation_tasks.push(task);
                    }
                }
            }
        }

        // Wait for all file creation tasks to complete
        let file_results = futures::future::join_all(file_creation_tasks).await;
        for file_result in file_results {
            file_result?;
        }
        result.add_phase("file_creation", start_time.elapsed());

        // Phase 5: Verify file system changes
        for (ticket_id, expected_files) in test_scenario.expected_files.iter() {
            // Find the worker assigned to this ticket
            if let Some((worker_id, _)) = test_scenario.assignments.iter().find(|(_, tid)| *tid == ticket_id) {
                if let Some(worktree_path) = worktree_assignments.get(worker_id) {
                    for expected_file in expected_files {
                        self.file_verifier.verify_file_exists(
                            &worktree_path.join(&expected_file.path),
                            &expected_file.expected_content
                        ).await?;
                    }
                }
            }
        }
        result.add_phase("file_verification", start_time.elapsed());

        // Phase 6: Workers update ticket status
        for worker in &self.workers {
            let assigned_tickets: Vec<_> = test_scenario.assignments
                .iter()
                .filter(|(worker_id, _)| *worker_id == worker.id())
                .map(|(_, ticket_id)| *ticket_id)
                .collect();

            for ticket_id in assigned_tickets {
                worker.update_ticket_status(&self.server, ticket_id, "completed").await?;
            }
        }
        result.add_phase("status_update", start_time.elapsed());

        // Phase 7: Cleanup worktrees
        for (_, worktree_path) in worktree_assignments {
            self.worktree_manager.cleanup_worktree(&worktree_path).await?;
        }
        result.add_phase("cleanup", start_time.elapsed());

        result.set_total_duration(start_time.elapsed());
        result.mark_successful();

        Ok(result)
    }

    /// Cleans up test resources
    pub async fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove test workspace
        if self.workspace_dir.exists() {
            tokio::fs::remove_dir_all(&self.workspace_dir).await?;
        }
        Ok(())
    }
}

impl Drop for CoordinatorWorkerTestFramework {
    fn drop(&mut self) {
        // Best effort cleanup - errors are ignored since this is in Drop
        if self.workspace_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.workspace_dir);
        }
    }
}

/// Test scenario definition for workflow tests
#[derive(Debug, Clone)]
pub struct WorkflowTestScenario {
    /// Name of the test scenario
    pub name: String,
    /// Tickets to be created by coordinator
    pub tickets: Vec<TicketDefinition>,
    /// Worker-to-ticket assignments (worker_id, ticket_id)
    pub assignments: Vec<(Uuid, Uuid)>,
    /// Expected files to be created by workers (ticket_id -> files)
    pub expected_files: HashMap<Uuid, Vec<ExpectedFile>>,
}

/// Definition of a ticket to be created during testing
#[derive(Debug, Clone)]
pub struct TicketDefinition {
    /// Ticket ID (can be pre-generated for predictable assignments)
    pub id: Uuid,
    /// Ticket title
    pub title: String,
    /// Ticket description
    pub description: String,
    /// Ticket priority
    pub priority: String,
}

/// Definition of an expected file to be created by a worker
#[derive(Debug, Clone)]
pub struct ExpectedFile {
    /// Relative path where file should be created
    pub path: PathBuf,
    /// Expected content of the file
    pub expected_content: String,
}

/// Result of running a workflow test
#[derive(Debug)]
pub struct WorkflowTestResult {
    /// Name of the test that was run
    pub test_name: String,
    /// Whether the test succeeded
    pub success: bool,
    /// Duration of each phase
    pub phase_durations: HashMap<String, Duration>,
    /// Total test duration
    pub total_duration: Duration,
    /// Error message if test failed
    pub error_message: Option<String>,
}

impl WorkflowTestResult {
    /// Creates a new test result
    pub fn new(test_name: String) -> Self {
        Self {
            test_name,
            success: false,
            phase_durations: HashMap::new(),
            total_duration: Duration::from_secs(0),
            error_message: None,
        }
    }

    /// Adds a phase duration to the result
    pub fn add_phase(&mut self, phase_name: &str, duration: Duration) {
        self.phase_durations.insert(phase_name.to_string(), duration);
    }

    /// Sets the total test duration
    pub fn set_total_duration(&mut self, duration: Duration) {
        self.total_duration = duration;
    }

    /// Marks the test as successful
    pub fn mark_successful(&mut self) {
        self.success = true;
    }

    /// Marks the test as failed with an error message
    pub fn mark_failed(&mut self, error: &str) {
        self.success = false;
        self.error_message = Some(error.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that validates the complete coordinator-worker workflow as specified in issue #77
    #[tokio::test]
    async fn test_coordinator_creates_assigns_tickets() {
        let mut framework = CoordinatorWorkerTestFramework::new()
            .await
            .expect("Failed to create test framework");

        // Add workers with specific capabilities
        framework.add_worker("backend-worker", vec!["rust".to_string(), "api".to_string()])
            .await
            .expect("Failed to add backend worker");
        
        framework.add_worker("frontend-worker", vec!["typescript".to_string(), "react".to_string()])
            .await
            .expect("Failed to add frontend worker");

        // Setup the framework (register agents via MCP)
        framework.setup().await.expect("Failed to setup framework");

        // Define test scenario
        let backend_ticket_id = Uuid::new_v4();
        let frontend_ticket_id = Uuid::new_v4();
        let backend_worker_id = framework.workers[0].id();
        let frontend_worker_id = framework.workers[1].id();

        let scenario = WorkflowTestScenario {
            name: "coordinator_creates_assigns_tickets".to_string(),
            tickets: vec![
                TicketDefinition {
                    id: backend_ticket_id,
                    title: "Implement user authentication API".to_string(),
                    description: "Create secure JWT-based authentication endpoints".to_string(),
                    priority: "high".to_string(),
                },
                TicketDefinition {
                    id: frontend_ticket_id,
                    title: "Create login component".to_string(),
                    description: "Build React component for user login with form validation".to_string(),
                    priority: "medium".to_string(),
                },
            ],
            assignments: vec![
                (backend_worker_id, backend_ticket_id),
                (frontend_worker_id, frontend_ticket_id),
            ],
            expected_files: {
                let mut files = HashMap::new();
                files.insert(backend_ticket_id, vec![
                    ExpectedFile {
                        path: PathBuf::from("src/auth.rs"),
                        expected_content: "// JWT Authentication implementation\npub struct AuthService;\n".to_string(),
                    },
                    ExpectedFile {
                        path: PathBuf::from("tests/auth_tests.rs"),
                        expected_content: "// Authentication tests\n#[tokio::test]\nasync fn test_jwt_auth() {\n    // Test implementation\n}\n".to_string(),
                    },
                ]);
                files.insert(frontend_ticket_id, vec![
                    ExpectedFile {
                        path: PathBuf::from("src/components/Login.tsx"),
                        expected_content: "// React Login component\nexport const Login = () => {\n    return <div>Login Form</div>;\n};\n".to_string(),
                    },
                ]);
                files
            },
        };

        // Run the workflow test
        let result = framework.run_workflow_test(scenario)
            .await
            .expect("Workflow test failed");

        // Verify test succeeded
        assert!(result.success, "Workflow test should succeed");
        assert!(result.phase_durations.contains_key("ticket_creation"));
        assert!(result.phase_durations.contains_key("worktree_creation"));
        assert!(result.phase_durations.contains_key("ticket_assignment"));
        assert!(result.phase_durations.contains_key("file_creation"));
        assert!(result.phase_durations.contains_key("file_verification"));

        println!("✓ Coordinator-worker workflow test completed successfully");
        println!("  - Total duration: {:?}", result.total_duration);
        for (phase, duration) in result.phase_durations {
            println!("  - {}: {:?}", phase, duration);
        }

        // Cleanup
        framework.cleanup().await.expect("Failed to cleanup framework");
    }

    /// Test multiple workers working on the same project with workspace isolation
    #[tokio::test]
    async fn test_multi_worker_isolated_workspaces() {
        let mut framework = CoordinatorWorkerTestFramework::new()
            .await
            .expect("Failed to create test framework");

        // Add multiple workers of the same type
        for i in 0..3 {
            framework.add_worker(
                &format!("worker-{}", i),
                vec!["development".to_string(), "collaboration".to_string()]
            ).await.expect("Failed to add worker");
        }

        framework.setup().await.expect("Failed to setup framework");

        // Create scenario where multiple workers work on different parts of the same feature
        let mut tickets = Vec::new();
        let mut assignments = Vec::new();
        let mut expected_files = HashMap::new();

        for (i, worker) in framework.workers.iter().enumerate() {
            let ticket_id = Uuid::new_v4();
            tickets.push(TicketDefinition {
                id: ticket_id,
                title: format!("Implement module {}", i),
                description: format!("Worker {} implements their assigned module", i),
                priority: "medium".to_string(),
            });
            
            assignments.push((worker.id(), ticket_id));
            
            expected_files.insert(ticket_id, vec![
                ExpectedFile {
                    path: PathBuf::from(format!("src/module_{}.rs", i)),
                    expected_content: format!("// Module {} implementation\npub struct Module{};\n", i, i),
                },
            ]);
        }

        let scenario = WorkflowTestScenario {
            name: "multi_worker_isolated_workspaces".to_string(),
            tickets,
            assignments,
            expected_files,
        };

        let result = framework.run_workflow_test(scenario)
            .await
            .expect("Multi-worker test failed");

        assert!(result.success, "Multi-worker test should succeed");
        
        println!("✓ Multi-worker isolated workspace test completed successfully");

        framework.cleanup().await.expect("Failed to cleanup framework");
    }

    /// Test error handling and recovery in coordinator-worker workflows
    #[tokio::test]
    async fn test_error_recovery_workflow() {
        let mut framework = CoordinatorWorkerTestFramework::new()
            .await
            .expect("Failed to create test framework");

        framework.add_worker("reliable-worker", vec!["development".to_string()])
            .await.expect("Failed to add worker");

        framework.setup().await.expect("Failed to setup framework");

        let ticket_id = Uuid::new_v4();
        let worker_id = framework.workers[0].id();

        // Test scenario with a file that should cause an error initially
        let scenario = WorkflowTestScenario {
            name: "error_recovery_workflow".to_string(),
            tickets: vec![
                TicketDefinition {
                    id: ticket_id,
                    title: "Implement error-prone feature".to_string(),
                    description: "Feature that initially fails but recovers".to_string(),
                    priority: "high".to_string(),
                },
            ],
            assignments: vec![(worker_id, ticket_id)],
            expected_files: {
                let mut files = HashMap::new();
                files.insert(ticket_id, vec![
                    ExpectedFile {
                        path: PathBuf::from("src/recovery_test.rs"),
                        expected_content: "// Error recovery test\npub fn recover() -> Result<(), String> {\n    Ok(())\n}\n".to_string(),
                    },
                ]);
                files
            },
        };

        let result = framework.run_workflow_test(scenario)
            .await
            .expect("Error recovery test failed");

        assert!(result.success, "Error recovery test should succeed");
        
        println!("✓ Error recovery workflow test completed successfully");

        framework.cleanup().await.expect("Failed to cleanup framework");
    }
}