# Project Configuration Guide

This document explains how to configure projects in the vibe-ensemble-mcp system, including the project.rules and project.patterns fields.

## Overview

Projects in vibe-ensemble-mcp support two key configuration fields that define how workers should behave within the project:

- **`project.rules`**: Project-specific rules and policies that workers must follow
- **`project.patterns`**: Code patterns, conventions, and architectural guidelines

These fields are stored in the database and automatically retrieved by workers when they start processing tickets.

## Project Rules (project.rules)

The `project.rules` field defines mandatory policies and procedures that all workers must follow. This is particularly important for:

- Git workflow requirements
- Security policies
- Quality standards
- Communication protocols
- Error handling procedures

### Git Rules Template

All projects should include comprehensive git rules to ensure consistent version control practices:

```markdown
# Git Workflow Rules

## Branch Policy
- **Main branches**: Use 'main' or 'develop' as base branches only
- **Feature branches**: Create feature branches for complex development (Phase 2)
- **Branch validation**: Workers must verify correct branch before starting work

## Commit Requirements (MANDATORY)
All workers that modify files MUST commit their changes before completion.

### Conventional Commits (STRICT)
- Format: `type: description`
- Types: feat, fix, docs, test, refactor, chore, perf, style
- Lowercase, imperative mood ("add" not "adds")
- Maximum 50 characters for subject line
- NO attribution (no Co-authored-by, signatures, or author credits)

### Examples by Worker Type:
**Implementation Workers:**
- `feat: implement user authentication system`
- `fix: resolve memory leak in worker process`
- `refactor: simplify database connection logic`

**Testing Workers:**
- `test: add unit tests for authentication`
- `test: add integration tests for API endpoints`
- `fix: resolve race condition found in testing`

**Documentation Workers:**
- `docs: add API documentation with examples`
- `docs: update installation guide`
- `fix: correct broken links in user guide`

## Git Conflict Policy
If workers encounter:
- Uncommitted changes from previous work
- Merge conflicts
- Git operation failures

**REQUIRED ACTION**:
1. STOP immediately
2. Use `coordinator_attention` outcome
3. Provide detailed explanation of the git issue
4. Do NOT attempt to resolve conflicts independently

## File Management
- **Stage ALL changes**: Always use `git add .` to include new files
- **Verify commits**: Check `git log --oneline -1` after committing
- **Clean status**: Ensure `git status` shows clean working directory before starting

## Error Recovery
- Failed commits → coordinator_attention outcome
- Permission errors → coordinator_attention outcome
- Repository corruption → coordinator_attention outcome
```

### Security Rules Template

```markdown
# Security Rules

## Credential Management
- NEVER commit secrets, API keys, or passwords
- Use environment variables for sensitive configuration
- Implement secure credential rotation practices

## Data Protection
- Validate all user inputs
- Sanitize outputs to prevent injection attacks
- Implement proper error handling without information disclosure

## Access Control
- Follow principle of least privilege
- Implement proper authentication and authorization
- Log security-relevant events
```

### Quality Standards Template

```markdown
# Quality Standards

## Code Quality
- Maintain test coverage above 80%
- Follow project-specific coding standards
- Implement comprehensive error handling
- Add appropriate logging for debugging

## Documentation Requirements
- Document all public APIs
- Maintain up-to-date README files
- Include inline comments for complex logic
- Provide usage examples

## Testing Requirements
- Unit tests for all new functionality
- Integration tests for API endpoints
- Performance tests for critical paths
- Security tests for sensitive features
```

## Project Patterns (project.patterns)

The `project.patterns` field defines architectural patterns, code conventions, and development guidelines:

```markdown
# Development Patterns

## Architecture Patterns
- Use dependency injection for testability
- Implement repository pattern for data access
- Follow MVC/MVP patterns for UI components
- Use event-driven architecture for loose coupling

## Code Conventions
- Use descriptive variable and function names
- Implement consistent error handling patterns
- Follow language-specific style guides
- Maintain consistent file and directory structure

## Framework-Specific Patterns
[Include patterns specific to your technology stack]

## Database Patterns
- Use migrations for schema changes
- Implement proper indexing strategies
- Follow normalization principles
- Use connection pooling for performance
```

## Setting Project Configuration

### During Project Creation

```bash
# Example: Creating project with comprehensive rules
create_project(
    repository_name="my-project",
    path="/path/to/project",
    description="Project description",
    rules="[Include git rules, security rules, quality standards]",
    patterns="[Include architecture patterns, code conventions]"
)
```

### Updating Existing Projects

```bash
# Update project rules
update_project(
    repository_name="my-project",
    rules="[Updated rules content]"
)

# Update project patterns
update_project(
    repository_name="my-project",
    patterns="[Updated patterns content]"
)
```

## Worker Integration

### How Workers Access Configuration

Workers automatically retrieve project configuration during initialization:

1. **Rule Retrieval**: Workers call `get_project_rules()` to access current rules
2. **Pattern Retrieval**: Workers call `get_project_patterns()` to access patterns
3. **Version Tracking**: System tracks rule/pattern versions for consistency

### Rule Enforcement

- **Mandatory Compliance**: All workers MUST follow project.rules
- **Pattern Guidelines**: Workers SHOULD follow project.patterns when applicable
- **Validation**: System validates rule compliance during worker completion
- **Escalation**: Rule violations trigger coordinator attention

## Best Practices

### Rule Definition
- Keep rules concise but comprehensive
- Use clear, actionable language
- Include examples and templates
- Regular review and updates

### Pattern Guidelines
- Focus on architectural consistency
- Provide code examples
- Link to external style guides
- Document rationale for patterns

### Maintenance
- Version control rule changes
- Communicate updates to team
- Regular reviews for effectiveness
- Archive deprecated patterns

## Example Complete Configuration

```markdown
# Complete project.rules example
[Git Workflow Rules - see template above]
[Security Rules - see template above]
[Quality Standards - see template above]

# Custom project-specific rules
## Project-Specific Requirements
- All API changes require backward compatibility
- Database migrations must be reversible
- Critical path code requires peer review
- Performance impact assessments for new features
```

This configuration ensures consistent, high-quality development across all workers while maintaining flexibility for project-specific requirements.