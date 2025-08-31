# Contributing to Vibe Ensemble

Thank you for your interest in contributing to Vibe Ensemble! This guide will help you get started with contributing to our local Claude Code coordination system.

## Getting Started

### Prerequisites

- **Rust 1.80+** with Cargo
- **Git** for version control
- **Claude Code** for testing coordination features

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
   cargo test --workspace
   ```

4. **Run code quality checks**:
   ```bash
   cargo fmt
   cargo clippy
   RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features
   ```

## Development Workflow

### Git Worktrees for Parallel Development

This project supports **git worktrees** for parallel development:

```bash
# Create a worktree for your feature
git worktree add -b feature/your-feature-name ../vibe-ensemble-feature

# Work in the new directory
cd ../vibe-ensemble-feature

# When done, clean up
git worktree remove ../vibe-ensemble-feature
```

See our [Git Worktrees Guide](docs/git-worktrees.md) for detailed information.

### Branch Naming Conventions

- **Features**: `feature/descriptive-name`
- **Bug fixes**: `fix/issue-description`
- **Documentation**: `docs/topic-name`
- **Refactoring**: `refactor/component-name`

### Commit Message Format

Use **single line commit messages**:

```
feat: implement agent registration system
fix: resolve connection timeout in messaging
docs: add installation guide
refactor: simplify error handling
test: add integration tests for MCP protocol
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

## Code Standards

### Rust Guidelines

- **Follow Rust idioms** and best practices
- **Use ownership patterns** effectively
- **Implement explicit error handling** with `Result` types
- **Leverage async/await** for concurrent operations
- **Write comprehensive tests** for all new functionality
- **Include documentation** for public APIs

### Code Quality Requirements

Before committing, ensure:

1. **All tests pass**: `cargo test --workspace`
2. **No clippy warnings**: `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features`
3. **Code formatted**: `cargo fmt`
4. **Project builds**: `cargo build`

**No exceptions** - all quality checks must pass before committing.

### Error Handling

Use `Result<T, E>` for fallible operations:

```rust
pub fn register_agent(&self, agent: Agent) -> Result<AgentId, AgentError> {
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

3. **Component Tests**: Test full coordination workflows
   ```bash
   cargo test --workspace
   ```

### Writing Tests

- **Test public interfaces** thoroughly
- **Use descriptive test names** that explain the scenario
- **Include both success and failure cases**
- **Test edge cases** and boundary conditions

Example test:
```rust
#[tokio::test]
async fn test_agent_registration_success() {
    let manager = AgentManager::new();
    let agent = Agent::new("test-agent", vec!["frontend"]);
    
    let result = manager.register_agent(agent).await;
    
    assert!(result.is_ok());
    let agent_id = result.unwrap();
    assert!(manager.get_agent(&agent_id).await.is_some());
}
```

## Documentation

### Documentation Requirements

- **Public APIs** must have rustdoc comments
- **Examples** for complex functionality
- **Architecture decisions** documented in `docs/`

### Documentation Style

```rust
/// Registers a new agent with the coordination system.
///
/// This validates the agent configuration and stores it in SQLite.
///
/// # Arguments
///
/// * `agent` - The agent configuration to register
///
/// # Returns
///
/// Returns the assigned agent ID on success.
///
/// # Examples
///
/// ```rust
/// let agent = Agent::new("worker-1", vec!["frontend"]);
/// let agent_id = manager.register_agent(agent).await?;
/// ```
pub async fn register_agent(&self, agent: Agent) -> Result<AgentId, AgentError> {
    // Implementation
}
```

## Pull Request Process

### Before Submitting

1. **Ensure all tests pass**: `cargo test --workspace`
2. **Run quality checks**: `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features`
3. **Format code**: `cargo fmt`
4. **Update documentation** if needed
5. **Add tests** for new functionality

### Pull Request Guidelines

- **Descriptive title** summarizing the change
- **Detailed description** explaining what and why
- **Link to related issues** using keywords (fixes #123)
- **Test plan** describing verification steps

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
- [ ] Manual testing with Claude Code

Fixes #123
```

## Security Guidelines

### Secure Coding Practices

- **Never commit secrets** or sensitive data
- **Validate all inputs** from external sources
- **Handle user data** responsibly
- **Follow principle of least privilege**

### Reporting Security Issues

- **Do not** create public issues for security vulnerabilities
- **Email** security concerns to maintainers privately
- **Include** detailed description and reproduction steps

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

- [User Guide](docs/user-guide.md) - How to use Vibe Ensemble
- [Developer Guide](docs/developer-guide.md) - Technical architecture
- [Installation Guide](docs/installation.md) - Setup instructions
- [Git Worktrees Guide](docs/git-worktrees.md) - Parallel development

### Questions and Support

- **Check existing documentation** first
- **Search closed issues** for similar problems
- **Create detailed issue** if you can't find an answer
- **Join discussions** for broader questions

## Release Process

Contributors don't need to handle releases - maintainers will:

1. Update version numbers
2. Create release tags
3. Publish to GitHub releases
4. Update documentation

## Licensing

By contributing, you agree that your contributions will be licensed under the Apache License 2.0, the same license as the project.

---

Thank you for contributing to Vibe Ensemble! Your efforts help make Claude Code coordination better for everyone.