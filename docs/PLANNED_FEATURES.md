# Planned Features

This document outlines features planned for future development in the vibe-ensemble-mcp system.

## Advanced Git Integration (Phase 2)

**Status**: Planned for future implementation
**Prerequisites**: Phase 1 Git Integration (basic repository management, commit workflows, conflict detection)

### Overview
Advanced git workflow management capabilities that extend beyond basic commit tracking to provide comprehensive version control integration for multi-agent development workflows.

### Features

#### 1. Git Commit Audit Events
- **Description**: Integrate git commits into the event system for complete audit trails
- **Benefits**:
  - Track which worker made which commits across all projects
  - Correlate commits with worker completion events
  - Enable rollback tracking and change attribution at the system level
- **Implementation**: Emit events when workers successfully commit changes, including commit hash, worker ID, and conventional commit message

#### 2. Branch Management for Complex Workflows
- **Description**: Intelligent branch management for feature development and parallel work streams
- **Features**:
  - Automatic feature branch creation for complex tickets
  - Branch merging strategies when tickets complete
  - Parallel development coordination across multiple tickets
  - Integration with ticket dependencies and blocking relationships
- **Use Cases**: Large features spanning multiple stages, hotfix workflows, experimental development

#### 3. Pre-commit Hooks Integration
- **Description**: Automated validation and formatting before commits
- **Features**:
  - Code formatting enforcement (language-specific)
  - Lint checking and automated fixes
  - Conventional commit message validation at git level
  - Custom project-specific validation rules
- **Integration**: Hook into existing worker validation workflows

#### 4. Remote Repository Integration
- **Description**: Seamless integration with remote git repositories (GitHub, GitLab, etc.)
- **Features**:
  - Automatic pushing of completed work
  - Pull request/merge request creation for completed tickets
  - Remote conflict detection and resolution workflows
  - Integration with CI/CD pipelines
  - Synchronization with remote changes during development

### Technical Considerations

#### Security
- Secure credential management for remote repositories
- Permission validation for push/pull operations
- Isolation of worker git operations

#### Performance
- Efficient git operations that don't block worker processing
- Background synchronization with remote repositories
- Optimized git status checking and change detection

#### Reliability
- Recovery mechanisms for failed git operations
- Rollback capabilities for problematic commits
- Backup strategies for local git repositories

### Integration Points

#### Event System
- New event types: `git_commit_created`, `branch_created`, `merge_completed`, `remote_sync_completed`
- Enhanced event metadata with git-specific information
- Correlation between git events and worker lifecycle events

#### Worker Templates
- Enhanced git workflow instructions
- Branch-aware development patterns
- Remote repository interaction guidelines
- Advanced conflict resolution strategies

#### Coordinator Tools
- Git repository management tools
- Branch visualization and management
- Remote repository configuration and monitoring
- Git history analysis and reporting

### Success Metrics
- Reduced git-related worker failures
- Improved traceability of changes across development cycles
- Seamless integration with existing development workflows
- Enhanced collaboration capabilities for multi-agent development

### Dependencies
- Completion of Phase 1 Git Integration
- Enhanced worker template system
- Extended event system capabilities
- Remote repository access and credentials management

## Automatic Update Tracking (Phase 1)

**Status**: Planned for immediate implementation
**Prerequisites**: None (builds on existing event system)

### Overview
Built-in periodic update checking system that generates dedicated events for software update tracking. Provides operational intelligence about available updates without user interaction, leveraging the existing event infrastructure for comprehensive tracking and monitoring.

### Core Functionality

#### 1. **Periodic Update Checker Service**
- **Component**: Background service integrated into the server startup sequence
- **Check Interval**: Configurable (default: 24 hours), with immediate check on first startup
- **Update Source**: GitHub Releases API (`https://api.github.com/repos/siy/vibe-ensemble-mcp/releases/latest`)
- **Rate Limiting**: Respects GitHub API rate limits with exponential backoff
- **Caching**: Results cached to avoid redundant API calls

#### 2. **Event System Integration**
- **New Event Types**:
  - `UpdateCheckStarted`: When update check begins
  - `UpdateCheckCompleted`: When check completes successfully
  - `UpdateCheckFailed`: When check fails (network, API errors)
  - `UpdateAvailable`: When new version is detected
  - `UpdateNoChange`: When already on latest version

#### 3. **Event Data Structure**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventData {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub check_duration_ms: u64,
    pub last_check: DateTime<Utc>,
    pub release_url: Option<String>,
    pub release_notes_url: Option<String>,
}
```

### Implementation Components

#### 1. **Update Service Module** (`src/updates/mod.rs`)
```rust
pub struct UpdateService {
    current_version: String,
    github_repo: String,
    check_interval: Duration,
    last_check: Option<DateTime<Utc>>,
    http_client: reqwest::Client,
}

impl UpdateService {
    pub async fn check_for_updates(&mut self) -> Result<UpdateInfo>
    pub async fn start_periodic_checks(&self, event_emitter: EventEmitter)
    pub fn should_check(&self) -> bool
}
```

#### 2. **Configuration Extension**
- Add CLI flag: `--update-check-interval-hours` (default: 24)
- Add CLI flag: `--disable-update-checks` (default: false)
- Configuration stored in `Config` struct

#### 3. **Server Integration Points**
- **Startup**: Initialize update service and perform immediate check
- **Background Task**: Spawn long-running task for periodic checks
- **Event Emission**: Integrate with existing event emitter system
- **Health Check**: Include update status in `/health` endpoint

#### 4. **Database Storage**
- Leverage existing events table for historical tracking
- No new database schema required
- Events are automatically persisted and queryable

### Event Flow

```
1. Server Startup
   ↓
2. UpdateService.check_for_updates()
   ↓
3. Emit UpdateCheckStarted event
   ↓
4. GitHub API call
   ↓
5. Parse response & compare versions
   ↓
6. Emit appropriate completion event:
   - UpdateAvailable (with version info)
   - UpdateNoChange
   - UpdateCheckFailed (on error)
   ↓
7. Schedule next check
```

### CLI Configuration

```bash
# Default behavior (24-hour checks)
./vibe-ensemble-mcp --port 3276

# Custom check interval
./vibe-ensemble-mcp --port 3276 --update-check-interval-hours 6

# Disable update checks entirely
./vibe-ensemble-mcp --port 3276 --disable-update-checks

# Manual check trigger via new MCP tool
# (coordinators can call mcp__vibe-ensemble-mcp__check_updates)
```

### Health Check Integration

```json
{
  "status": "healthy",
  "service": "vibe-ensemble-mcp",
  "timestamp": "2025-01-15T10:30:00Z",
  "database": { "version": "..." },
  "update_status": {
    "current_version": "0.9.5",
    "latest_version": "0.9.6",
    "update_available": true,
    "last_checked": "2025-01-15T09:30:00Z",
    "next_check": "2025-01-16T09:30:00Z"
  }
}
```

### Event Examples

```json
{
  "event_type": "update_check_started",
  "timestamp": "2025-01-15T10:00:00Z",
  "data": {
    "System": {
      "component": "update_service",
      "message": "Starting update check",
      "metadata": {
        "current_version": "0.9.5",
        "check_interval_hours": 24
      }
    }
  }
}

{
  "event_type": "update_available",
  "timestamp": "2025-01-15T10:00:15Z",
  "data": {
    "System": {
      "component": "update_service",
      "message": "Update available: v0.9.6",
      "metadata": {
        "current_version": "0.9.5",
        "latest_version": "0.9.6",
        "update_available": true,
        "release_url": "https://github.com/siy/vibe-ensemble-mcp/releases/tag/v0.9.6",
        "release_notes_url": "https://github.com/siy/vibe-ensemble-mcp/releases/tag/v0.9.6",
        "check_duration_ms": 850
      }
    }
  }
}
```

### Benefits

#### 1. **Operational Intelligence**
- Historical tracking of update availability
- Performance monitoring of update checks
- Integration with existing monitoring infrastructure

#### 2. **Automation Ready**
- Events can trigger automated workflows
- Coordinators can programmatically query update status
- Foundation for future automated update systems

#### 3. **Zero User Disruption**
- Completely background operation
- No interruption to worker processes
- Optional and configurable

#### 4. **Monitoring Integration**
- SSE events enable real-time monitoring dashboards
- Health check provides current status
- Event history available via existing tools

### Implementation Notes

#### 1. **Error Handling**
- Network failures logged but don't affect server operation
- API rate limit handling with exponential backoff
- Graceful degradation if GitHub is unavailable

#### 2. **Performance Considerations**
- Non-blocking background checks
- Minimal memory footprint
- Efficient HTTP client with connection pooling

#### 3. **Security**
- No credentials required (public GitHub API)
- HTTPS-only connections
- No sensitive data exposure

#### 4. **Future Extension Points**
- Support for pre-release tracking
- Custom release channels
- Integration with package managers
- Automated update workflows

### Required Changes Summary

1. **New Module**: `src/updates/mod.rs` - Core update checking logic
2. **Event System**: Add new event types to `src/events/mod.rs`
3. **Configuration**: Extend `src/config.rs` and CLI args in `src/main.rs`
4. **Server Integration**: Modify `src/server.rs` for startup and health check
5. **MCP Tools**: Optional coordinator tools for manual update checks
6. **Dependencies**: Add `reqwest` for HTTP client

### Technical Considerations

#### Integration with Event System
- New event types will be added to the existing strongly-typed event system
- Leverages existing SSE broadcasting for real-time notifications
- Integrates with existing database storage for historical tracking

#### Worker Template Changes
- No changes required to existing worker templates
- Optional: Enhance system prompts to include update awareness
- Workers remain isolated from update checking operations

#### Success Metrics
- Reliable detection of available updates within configured intervals
- Zero impact on worker processing performance
- Comprehensive event tracking for operational visibility
- Successful integration with existing monitoring tools

This feature provides comprehensive update tracking while maintaining the system's non-intrusive design philosophy and leveraging existing infrastructure for maximum integration value.