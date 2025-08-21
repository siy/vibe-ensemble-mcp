# Contributing Guide

This guide provides detailed information about contributing to the Vibe Ensemble MCP Server project, including development workflows, code standards, and community guidelines.

## Quick Start for Contributors

### Prerequisites
- Rust 1.70+ with Cargo
- Git for version control
- SQLite for development database
- Claude Code for testing agent coordination features

### First-time Setup
```bash
# Fork and clone the repository
git clone git@github.com:<your-username>/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build and test
cargo build
cargo test --all

# Run code quality checks
cargo fmt
cargo clippy
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features
```

## Development Workflow

### Git Worktrees for Parallel Development

This project supports and encourages the use of **git worktrees** for parallel development:

```bash
# Create a worktree for your feature
git worktree add -b feature/your-feature-name ../vibe-ensemble-feature

# Work in the new directory
cd ../vibe-ensemble-feature

# When done, clean up
git worktree remove ../vibe-ensemble-feature
```

See our [Git Worktrees Guide](../git-worktrees.md) for detailed information.

### Branch Naming Conventions

- **Features**: `feature/descriptive-name`
- **Bug fixes**: `fix/issue-description`
- **Documentation**: `docs/topic-name`
- **Refactoring**: `refactor/component-name`
- **Tests**: `test/component-name`
- **Chores**: `chore/task-description`

### Commit Message Format

Follow the **single line convenient commits convention**:

```
type: brief description

Examples:
feat: implement agent registration system
fix: resolve connection timeout in messaging
docs: add git worktree usage guide
refactor: simplify error handling in storage layer
test: add integration tests for MCP protocol
chore: update dependencies to latest versions
```

**Commit Types**:
- `feat`: New features
- `fix`: Bug fixes  
- `docs`: Documentation changes
- `style`: Code style changes (no logic changes)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Build process or auxiliary tool changes

## Code Standards

### Rust Guidelines

#### Idiomatic Rust
- Follow Rust idioms and best practices
- Use ownership patterns and lifetimes effectively
- Leverage the type system for correctness
- Implement explicit error handling with `Result` types
- Use async/await for concurrent operations

#### Code Structure
```rust
// Good: Clear module organization
pub mod agent {
    pub mod registration;
    pub mod lifecycle;
    pub mod capabilities;
}

// Good: Explicit error handling
pub fn register_agent(agent: Agent) -> Result<AgentId, AgentError> {
    validate_agent(&agent)?;
    let id = self.storage.store_agent(agent)
        .map_err(AgentError::StorageFailure)?;
    Ok(id)
}

// Good: Documentation with examples
/// Registers a new agent with the coordination system.
///
/// # Examples
/// ```rust
/// let agent = Agent::new("worker-1", vec!["code-review"]);
/// let agent_id = manager.register_agent(agent).await?;
/// ```
pub async fn register_agent(&self, agent: Agent) -> Result<AgentId, AgentError> {
    // Implementation
}
```

### Code Quality Requirements

#### Mandatory Checks
- **Test coverage**: Must be above 90%
- **All clippy warnings**: Must be resolved
- **Code formatting**: Must pass `cargo fmt --check`
- **No compiler warnings**: In strict mode (`RUSTFLAGS="-D warnings"`)
- **Documentation**: Required for all public APIs

#### Performance Guidelines
- Use appropriate data structures for the use case
- Avoid unnecessary allocations in hot paths
- Prefer borrowing over cloning when possible
- Use `async` only when necessary

### Error Handling Patterns

```rust
// Custom error types
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Invalid agent configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Storage operation failed: {0}")]
    StorageFailure(#[from] StorageError),
    #[error("Network communication failed: {0}")]
    NetworkFailure(#[from] NetworkError),
}

// Error propagation
pub async fn complex_operation(&self) -> Result<Success, AgentError> {
    let data = self.fetch_data().await?;
    let processed = self.process_data(data)?;
    self.store_result(processed).await?;
    Ok(Success)
}
```

## Testing Strategy

### Test Organization

```
tests/
├── unit/              # Unit tests for individual components
├── integration/       # Integration tests for service interactions
├── e2e/              # End-to-end tests for complete workflows
├── performance/      # Load and performance tests
├── fixtures/         # Test data and helpers
└── common/           # Shared test utilities
```

### Test Types and Commands

#### Unit Tests
Test individual components and functions:
```bash
# Run all unit tests
cargo test --lib

# Run specific module tests
cargo test --lib agent::tests

# Run with output
cargo test --lib -- --nocapture
```

#### Integration Tests
Test service interactions and database operations:
```bash
# Run integration tests
cargo test --test integration

# Run specific integration test
cargo test --test integration test_agent_registration
```

#### End-to-End Tests
Test complete user workflows:
```bash
# Run e2e tests
cargo test --test e2e

# Run e2e tests with real database
DATABASE_URL=postgres://user:pass@localhost/test cargo test --test e2e
```

#### Performance Tests
```bash
# Run performance benchmarks
cargo bench

# Run load tests
cargo test --test performance --release
```

### Writing Effective Tests

#### Test Structure
```rust
#[tokio::test]
async fn test_agent_registration_with_valid_data() {
    // Arrange
    let manager = AgentManager::new();
    let agent = Agent::new("test-agent", vec!["task-execution"]);
    
    // Act
    let result = manager.register_agent(agent).await;
    
    // Assert
    assert!(result.is_ok());
    let agent_id = result.unwrap();
    assert!(manager.get_agent(&agent_id).await.is_some());
}
```

#### Test Naming Conventions
- Use descriptive names: `test_<what>_<when>_<expected_outcome>`
- Examples:
  - `test_agent_registration_with_invalid_capabilities_returns_error`
  - `test_issue_assignment_when_no_agents_available_fails_gracefully`
  - `test_knowledge_search_with_empty_query_returns_all_entries`

#### Test Coverage Guidelines
- Test public interfaces thoroughly
- Include both success and failure cases
- Test edge cases and boundary conditions
- Mock external dependencies appropriately
- Use property-based testing for complex invariants

## Documentation Standards

### Documentation Types

#### API Documentation
All public APIs must have comprehensive rustdoc comments:

```rust
/// Registers a new agent with the coordination system.
///
/// This function validates the agent configuration, assigns a unique ID,
/// and stores the agent in the persistent storage layer. The agent will
/// be available for task assignment after successful registration.
///
/// # Arguments
///
/// * `agent` - The agent configuration including name, type, and capabilities
///
/// # Returns
///
/// Returns the assigned agent ID on success, or an error if registration fails.
///
/// # Errors
///
/// This function will return an error if:
/// - Agent configuration is invalid
/// - Storage operation fails
/// - Network communication fails
///
/// # Examples
///
/// ```rust
/// use vibe_ensemble_core::{Agent, AgentType};
/// 
/// let agent = Agent::new()
///     .with_name("worker-1")
///     .with_type(AgentType::Worker)
///     .with_capabilities(vec!["code-review", "testing"]);
/// 
/// let agent_id = manager.register_agent(agent).await?;
/// println!("Registered agent with ID: {}", agent_id);
/// ```
pub async fn register_agent(&self, agent: Agent) -> Result<AgentId, AgentError> {
    // Implementation
}
```

#### Architecture Documentation
- Keep architecture docs up-to-date with implementation
- Include diagrams for complex interactions
- Document design decisions and trade-offs
- Provide migration guides for breaking changes

#### User Documentation
- Write from user perspective
- Include step-by-step instructions
- Provide troubleshooting guidance
- Keep examples current and tested

## Pull Request Process

### Before Submitting

#### Pre-submission Checklist
- [ ] All tests pass: `cargo test --all`
- [ ] Code quality checks pass: `cargo clippy` and `cargo fmt --check`
- [ ] Documentation is updated for any API changes
- [ ] Tests are added for new functionality
- [ ] CHANGELOG.md is updated for user-facing changes
- [ ] Security considerations are addressed

#### Testing Your Changes
```bash
# Run the full test suite
cargo test --all

# Run with coverage
cargo tarpaulin --out html

# Run clippy with strict settings
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Check documentation
cargo doc --no-deps --document-private-items
```

### Pull Request Guidelines

#### PR Template
Use this template for your pull request:

```markdown
## Summary
Brief description of changes made and why they were necessary.

## Changes
- [ ] Added agent registration system with capability validation
- [ ] Implemented MCP protocol handlers for resource discovery
- [ ] Updated error handling to provide better user feedback
- [ ] Added comprehensive test coverage

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing with Claude Code agents completed
- [ ] Performance impact assessed
- [ ] Security implications reviewed

## Breaking Changes
- None
- OR: List any breaking changes with migration guide

## Documentation
- [ ] API documentation updated
- [ ] Architecture docs updated if needed
- [ ] User guide updated if needed
- [ ] Examples updated if needed

## Related Issues
Fixes #123
Closes #456
Related to #789
```

#### PR Best Practices
- **Atomic commits**: Each commit should represent a single logical change
- **Descriptive title**: Summarize the change clearly
- **Detailed description**: Explain what, why, and how
- **Link related issues**: Use keywords like "fixes", "closes", "resolves"
- **Responsive to feedback**: Address review comments promptly
- **Keep PRs focused**: Avoid mixing unrelated changes

### Review Process

#### Automated Checks
All PRs must pass:
- Continuous Integration (CI) builds
- Test suite execution
- Code formatting verification
- Lint checks
- Security scans

#### Code Review Guidelines
- Focus on logic, design, and maintainability
- Be constructive and specific in feedback
- Suggest alternatives when pointing out issues
- Approve when changes meet quality standards
- Request changes for significant issues

#### Addressing Feedback
- Respond to all review comments
- Make requested changes in new commits initially
- Squash commits before merge if requested
- Update PR description if scope changes

## Security Guidelines

### Secure Coding Practices

#### Data Protection
- Never commit secrets, passwords, or API keys
- Validate and sanitize all external inputs
- Use parameterized queries for database operations
- Implement proper authentication and authorization
- Handle sensitive data according to privacy requirements

#### Security Review Checklist
- [ ] No hardcoded secrets or credentials
- [ ] Input validation for all external data
- [ ] Proper error handling without information leakage
- [ ] Authentication/authorization checks in place
- [ ] Secure communication protocols used
- [ ] Dependencies are up-to-date and secure

### Reporting Security Issues

For security vulnerabilities:
1. **DO NOT** create public issues
2. Email security@vibe-ensemble.dev with details
3. Include reproduction steps and impact assessment
4. Wait for confirmation before any public disclosure
5. Work with maintainers on coordinated disclosure

## Community Guidelines

### Code of Conduct

#### Our Standards
- **Be respectful** and inclusive to all participants
- **Provide constructive** feedback and criticism
- **Help newcomers** get started and succeed
- **Focus on technical merit** in discussions
- **Assume good intentions** from all contributors

#### Unacceptable Behavior
- Harassment, discrimination, or offensive comments
- Personal attacks or trolling
- Publishing private information without permission
- Spam or excessive self-promotion

### Communication Channels

#### Primary Channels
- **GitHub Issues**: Bug reports, feature requests, and task tracking
- **GitHub Discussions**: Questions, ideas, and community conversations
- **Pull Requests**: Code contributions and technical discussions

#### Communication Best Practices
- Search existing issues/discussions before creating new ones
- Use clear, descriptive titles
- Provide sufficient context and details
- Be patient with response times
- Follow up appropriately on your issues

## Getting Help

### Resources

#### Documentation
- [Developer Setup Guide](setup.md) - Environment setup and development tools
- [Architecture Overview](architecture.md) - System design and components
- [API Documentation](../api/overview.md) - REST and MCP API reference
- [Troubleshooting Guide](../troubleshooting/common-issues.md) - Common problems and solutions

#### External Resources
- [Rust Documentation](https://doc.rust-lang.org/) - Language reference and guides
- [Tokio Documentation](https://docs.rs/tokio/) - Async runtime documentation
- [SQLx Documentation](https://docs.rs/sqlx/) - Database library documentation

### Questions and Support

#### Getting Help Effectively
1. **Check existing documentation** and search issues first
2. **Provide context** including error messages and environment details
3. **Create minimal reproduction** cases when possible
4. **Be specific** about what you've tried and what isn't working
5. **Follow up** with solutions or additional information

#### Support Channels
- **General questions**: GitHub Discussions
- **Bug reports**: GitHub Issues with `bug` label
- **Feature requests**: GitHub Issues with `enhancement` label
- **Security issues**: Private email to maintainers

## Release Process

### Version Management
- Follow [Semantic Versioning](https://semver.org/) (SemVer)
- Update CHANGELOG.md with user-facing changes
- Tag releases with version numbers
- Provide release notes with highlights

### Contribution Recognition

#### Attribution
All contributors are acknowledged through:
- README.md contributor section
- Git commit history preservation
- Release notes for significant contributions
- Special recognition for major features

#### Licensing
By contributing, you agree that your contributions will be licensed under the project's MIT license.

---

**Thank you for contributing to Vibe Ensemble!** Your efforts help make multi-agent coordination better for everyone. For any questions about this guide, please open a discussion or issue on GitHub.