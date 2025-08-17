# Git Worktrees for Multi-Agent Development

Git worktrees are a powerful feature that allows you to have multiple working directories connected to a single Git repository. In the context of Vibe Ensemble MCP server, worktrees enable multiple Claude Code worker agents to collaborate on the same project simultaneously without conflicts.

## What are Git Worktrees?

Git worktrees let you check out multiple branches of the same repository simultaneously in different directories. Instead of switching branches in a single working directory, you can have separate directories for different branches, enabling true parallel development.

## How This Enables Multi-Agent Collaboration

### Parallel Development
- **Multiple workers** can work on different features simultaneously
- **Branch isolation** prevents conflicts between concurrent development
- **Shared repository** maintains unified git history and metadata
- **Resource efficiency** avoids multiple repository clones

### Worker Agent Benefits
- **Context preservation** - each agent maintains its own working directory
- **Independent progress** - agents don't interfere with each other's work
- **Simplified merging** - clean branch separation reduces merge conflicts
- **Testing isolation** - agents can test changes without affecting other work

## Basic Git Worktree Commands

### Creating a New Worktree
```bash
# Create a new worktree for an existing branch
git worktree add ../project-feature-branch feature-branch

# Create a new worktree with a new branch
git worktree add -b new-feature ../project-new-feature

# Create a temporary worktree for experiments
git worktree add ../project-experiment --detach
```

### Managing Worktrees
```bash
# List all worktrees
git worktree list

# Remove a worktree directory first
rm -rf ../project-feature-branch

# Then prune the worktree reference
git worktree prune

# Or remove directly (Git 2.17+)
git worktree remove ../project-feature-branch
```

## Vibe Ensemble Integration

### Agent Coordination Workflow

1. **Coordinator Agent** creates worktrees for different features/tasks
2. **Worker Agents** are assigned to specific worktrees
3. **Isolated Development** proceeds in parallel
4. **Coordination** happens through the MCP server
5. **Integration** occurs when features are complete

### Recommended Directory Structure
```
project-main/                    # Main repository
├── .git/                        # Git metadata (shared)
└── src/                         # Main branch code

project-feature-auth/            # Worker 1: Authentication feature
├── src/                         # Feature branch code
└── .git -> ../project-main/.git # Linked to main repository

project-bugfix-validation/       # Worker 2: Bug fixes
├── src/                         # Bugfix branch code
└── .git -> ../project-main/.git # Linked to main repository

project-refactor-api/            # Worker 3: API refactoring
├── src/                         # Refactoring branch code
└── .git -> ../project-main/.git # Linked to main repository
```

## Best Practices for Agent Workflows

### 1. Descriptive Naming Conventions
Use clear, purpose-driven names for worktrees:
```bash
# Good: Indicates purpose and responsible agent
git worktree add ../myapp-agent1-auth-feature auth-feature
git worktree add ../myapp-agent2-api-refactor api-refactor

# Avoid: Generic or unclear names
git worktree add ../temp temp-branch
```

### 2. Agent Assignment Strategy
- **One worktree per agent** for primary development
- **Temporary worktrees** for experimentation and testing
- **Coordinator oversight** of worktree allocation and cleanup

### 3. Lifecycle Management
```bash
# Agent starts work
git worktree add -b agent1-feature-login ../project-agent1-login

# Agent completes work and creates PR
cd ../project-agent1-login
git push origin agent1-feature-login

# After merge, cleanup
git worktree remove ../project-agent1-login
git branch -d agent1-feature-login
```

### 4. Conflict Resolution
- **Branch isolation** minimizes conflicts
- **Regular syncing** with main branch
- **Coordinator mediation** for cross-cutting changes
- **Automated testing** in each worktree

## Common Use Cases

### A/B Testing Implementation Approaches
```bash
# Agent 1: Implementation A
git worktree add -b implement-feature-a ../project-impl-a

# Agent 2: Implementation B  
git worktree add -b implement-feature-b ../project-impl-b

# Compare and choose best approach
```

### Feature Development with Dependencies
```bash
# Agent 1: Core infrastructure
git worktree add -b core-infrastructure ../project-core

# Agent 2: Feature using infrastructure (waits for Agent 1)
git worktree add -b feature-on-core ../project-feature
```

### Hotfix Workflow
```bash
# Critical bug fix while feature development continues
git worktree add -b hotfix-critical-bug ../project-hotfix

# Quick fix, test, and deploy without disrupting feature work
```

## Integration with Vibe Ensemble

### MCP Server Coordination
- **Worktree allocation** through agent management resources
- **Status tracking** of work in each worktree
- **Conflict detection** and resolution assistance
- **Cleanup automation** after task completion

### Knowledge Management Integration
- **Best practices** for worktree usage stored in knowledge repository
- **Pattern recognition** for common worktree workflows
- **Automated suggestions** for optimal branch strategies

### Agent Configuration
- **Worktree preferences** in agent profiles
- **Directory naming conventions** in team standards
- **Cleanup policies** for temporary worktrees

This workflow enables true parallel development while maintaining the benefits of a unified repository and shared development history.