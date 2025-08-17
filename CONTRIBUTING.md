# Contributing to Vibe Ensemble MCP Server

Thank you for your interest in contributing to Vibe Ensemble! This guide will help you get started with contributing to our MCP server for Claude Code team coordination.

## Getting Started

### Prerequisites

- **Rust 1.70+** with Cargo
- **Git** for version control
- **SQLite** for development database
- **Claude Code** for testing agent coordination features

### Development Setup

1. **Fork and clone** the repository:
   ```bash
   git clone git@github.com:<your-username>/vibe-ensemble-mcp.git
   cd vibe-ensemble-mcp
   ```

2. **Build the project**:
   ```bash
   cargo build
   ```

3. **Run tests** to ensure everything works:
   ```bash
   cargo test --all
   ```

4. **Run code quality checks**:
   ```bash
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

See our [Git Worktrees Guide](docs/git-worktrees.md) for detailed information on using worktrees effectively.

### Branch Naming Conventions

- **Features**: `feature/descriptive-name`
- **Bug fixes**: `fix/issue-description`
- **Documentation**: `docs/topic-name`
- **Refactoring**: `refactor/component-name`

### Commit Message Format

Follow the **single line convenient commits convention**:

```
feat: implement agent registration system
fix: resolve connection timeout in messaging
docs: add git worktree usage guide
refactor: simplify error handling in storage layer
test: add integration tests for MCP protocol
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

## Code Standards

### Rust Guidelines

- **Follow Rust idioms** and best practices
- **Use ownership patterns** and lifetimes effectively
- **Implement explicit error handling** with `Result` types
- **Leverage async/await** for concurrent operations
- **Write comprehensive tests** for all new functionality
- **Include documentation** for public APIs

### Code Quality Requirements

- **Test coverage** must be above 90%
- **All clippy warnings** must be resolved
- **Code must be formatted** with `cargo fmt`
- **No compiler warnings** in strict mode
- **Documentation** required for public functions and modules

### Error Handling

- Use `Result<T, E>` for fallible operations
- Create custom error types when appropriate
- Provide meaningful error messages
- Handle errors gracefully with recovery strategies

Example:
```rust
pub fn register_agent(agent: Agent) -> Result<AgentId, AgentError> {
    validate_agent(&agent)?;
    let id = self.storage.store_agent(agent)
        .map_err(AgentError::StorageFailure)?;
    Ok(id)
}
```

## Testing Strategy

### Test Types

1. **Unit Tests**: Test individual components
   ```bash
   cargo test --lib
   ```

2. **Integration Tests**: Test MCP protocol compliance
   ```bash
   cargo test --test integration
   ```

3. **End-to-End Tests**: Multi-agent scenarios
   ```bash
   cargo test --test e2e
   ```

### Writing Tests

- **Test public interfaces** thoroughly
- **Mock external dependencies** appropriately
- **Use descriptive test names** that explain the scenario
- **Include both success and failure cases**
- **Test edge cases** and boundary conditions

Example test:
```rust
#[tokio::test]
async fn test_agent_registration_with_valid_data() {
    let manager = AgentManager::new();
    let agent = Agent::new("test-agent", vec!["task-execution"]);
    
    let result = manager.register_agent(agent).await;
    
    assert!(result.is_ok());
    let agent_id = result.unwrap();
    assert!(manager.get_agent(&agent_id).await.is_some());
}
```

## Documentation

### Documentation Requirements

- **Public APIs** must have rustdoc comments
- **Examples** should be included for complex functionality
- **Architecture decisions** should be documented in `docs/`
- **Configuration options** need comprehensive explanations

### Documentation Style

```rust
/// Registers a new agent with the coordination system.
///
/// This function validates the agent configuration, assigns a unique ID,
/// and stores the agent in the persistent storage layer.
///
/// # Arguments
///
/// * `agent` - The agent configuration to register
///
/// # Returns
///
/// Returns the assigned agent ID on success, or an error if registration fails.
///
/// # Examples
///
/// ```rust
/// let agent = Agent::new("worker-1", vec!["code-review", "testing"]);
/// let agent_id = manager.register_agent(agent).await?;
/// ```
pub async fn register_agent(&self, agent: Agent) -> Result<AgentId, AgentError> {
    // Implementation
}
```

## Issue Management

### Bug Reports

When reporting bugs, include:

- **Clear description** of the issue
- **Steps to reproduce** the problem
- **Expected vs. actual behavior**
- **Environment details** (Rust version, OS, etc.)
- **Error messages** and stack traces
- **Minimal reproduction case** if possible

### Feature Requests

For new features, provide:

- **Use case description** and motivation
- **Proposed solution** or approach
- **Alternative solutions** considered
- **Impact assessment** on existing functionality
- **Implementation complexity** estimate

## Pull Request Process

### Before Submitting

1. **Ensure all tests pass**: `cargo test --all`
2. **Run code quality checks**: `cargo clippy` and `cargo fmt`
3. **Update documentation** if needed
4. **Add tests** for new functionality
5. **Update CHANGELOG.md** if applicable

### Pull Request Guidelines

- **Descriptive title** summarizing the change
- **Detailed description** explaining what and why
- **Link to related issues** using keywords (fixes #123)
- **Test plan** describing how to verify the changes
- **Breaking changes** clearly documented

### PR Template

```markdown
## Summary
Brief description of changes made.

## Changes
- Added agent registration system
- Implemented MCP protocol handlers
- Updated error handling

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing with Claude Code agents

## Breaking Changes
None / List any breaking changes

Fixes #123
```

### Review Process

1. **Automated checks** must pass (CI/CD)
2. **Code review** by maintainers
3. **Address feedback** promptly
4. **Squash commits** if requested
5. **Merge** after approval

## Security Guidelines

### Secure Coding Practices

- **Never commit secrets** or sensitive data
- **Validate all inputs** from external sources
- **Use secure communication** protocols
- **Implement proper authentication** and authorization
- **Handle user data** responsibly

### Reporting Security Issues

- **Do not** create public issues for security vulnerabilities
- **Email** security concerns to the maintainers
- **Include** detailed description and reproduction steps
- **Wait** for confirmation before public disclosure

## Community Guidelines

### Code of Conduct

- **Be respectful** and inclusive
- **Provide constructive feedback**
- **Help newcomers** get started
- **Focus on technical merit**
- **Assume good intentions**

### Communication Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and community discussions
- **Pull Requests**: Code contributions and reviews

## Getting Help

### Resources

- [High-Level Design](docs/high-level-design.md) - System architecture
- [Implementation Plan](docs/implementation-plan.md) - Development roadmap
- [Git Worktrees Guide](docs/git-worktrees.md) - Parallel development workflow
- [Rust Documentation](https://doc.rust-lang.org/) - Language reference

### Questions and Support

- **Check existing issues** and documentation first
- **Search closed issues** for similar problems
- **Create detailed issue** if you can't find an answer
- **Join discussions** for broader questions

## Attribution

### Contributors

All contributors will be acknowledged in:

- **README.md** contributor section
- **Git commit history** (maintain authorship)
- **Release notes** for significant contributions

### Licensing

By contributing, you agree that your contributions will be licensed under the Apache License 2.0, the same license as the project.

---

Thank you for contributing to Vibe Ensemble! Your efforts help make multi-agent coordination better for everyone.