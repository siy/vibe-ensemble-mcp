# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.6] - 2025-09-30

### Added
- **🔄 Automatic Update Tracking**: Periodic update checks every 4 hours (configurable) with GitHub API integration
- **⬆️ One-Command Upgrade**: `vibe-ensemble-mcp --upgrade` for seamless updates via install script
- **📡 Update Events**: New event types (`UpdateCheckStarted`, `UpdateAvailable`, `UpdateCheckFailed`) integrated into event system
- **🛡️ Enhanced Validation**: Comprehensive input validation for worker spawning (ticket IDs, paths, prompts)
- **📋 Ticket On-Hold State**: Automatic placement of tickets on-hold when validation failures occur

### Fixed
- **🐛 Worker Spawn Race Condition**: Fixed double-claim issue preventing workers from starting (removed redundant claim in consumer)
- **📛 Project Name Display**: Fixed project_created event to use repository_name instead of non-existent id field
- **🔒 Scopeguard Safety**: Installed cleanup guard before fallible operations to prevent stuck claims
- **⚡ Async Cleanup**: Replaced Handle::block_on with tokio::spawn to avoid runtime deadlock
- **📝 Review Template**: Enhanced review worker template with comprehensive analysis requirements (59→243 lines)
- **🔕 Notification Cleanup**: Removed obsolete sampling/createMessage notifications
- **🔧 Worker Prompt**: Restored -p parameter for proper worker prompt input
- **✅ Path Validation**: Added project path validation at creation time

### Changed
- **📊 SSE Capacity**: Increased real-time notification capacity for better monitoring
- **🏥 Health Monitoring**: Added SSE/WebSocket connection health checks
- **🎯 Claim Feedback**: More explicit feedback when tickets cannot be claimed or are already claimed
- **⏱️ Configurable Timeout**: Worker execution timeout now configurable via WORKER_TIMEOUT_SECS
- **🔐 Safer DB Queries**: Improved database query handling and error recovery

### Dependencies
- Added `reqwest` for HTTP client functionality (GitHub API calls)

## [0.9.5] - 2025-09-29

### Fixed
- **🔧 Project Rules Injection**: Fixed project rules injection in worker prompts for consistent application of project standards
- **🎯 Manual Workflow Dispatch**: Added tag reference for manual workflow dispatch and improved CI release process
- **📦 Release Process**: Fixed CI to allow uploading binaries to existing releases

### Changed
- **🚀 Enhanced CI/CD**: Improved release workflow with manual dispatch capability for better release management

## [0.9.4] - 2025-09-29

### Fixed
- **🌐 Website Updates**: Fixed website and README.md content improvements
- **📚 Documentation**: Enhanced documentation clarity and accuracy

## [0.9.3] - 2025-09-29

### Added
- **🧩 Enhanced Worker Templates**: Improved worker template system with better customization support
- **🌐 Modern Website**: Added modern website infrastructure

### Fixed
- **📋 Template Management**: Improved worker template handling and loading mechanisms
- **🔧 Installation Process**: Fixed installation script and improved documentation quality

## [0.9.2] - 2025-09-29

### Added
- **🎨 Enhanced Worker Templates**: Comprehensive improvements to worker template system
- **🌐 Modern Website Infrastructure**: Added modern website for better project presentation

### Changed
- **📚 Documentation**: Improved documentation structure and content quality

## [0.9.0] - 2025-01-17

### Added
- **🧠 Task Breakdown Sizing Methodology**: Intelligent task breakdown with optimal context-performance optimization (~120K token budget per stage)
- **📐 Natural Boundary Detection**: Automatic task splitting along technology, functional, and expertise boundaries
- **⚡ Enhanced Planning Workers**: Built-in token estimation and pipeline optimization with comprehensive validation
- **📊 Real-Time SSE Integration**: Full Server-Sent Events protocol for live progress monitoring and event streaming
- **🔧 Improved Worker Templates**: 8 specialized worker templates with task sizing methodology integration
- **📋 Enhanced Coordinator Prompts**: Updated coordination with systematic task delegation and sizing guidance
- **🛠️ Robust MCP Tools**: 47 MCP tools with enhanced project metadata and worker coordination
- **📚 Comprehensive Documentation**: Complete SSE protocol implementation and task breakdown sizing methodology
- **🔒 Enhanced Security**: Permission system with three modes (bypass, inherit, file) for fine-grained worker access control

### Removed
- **Manual Ticket Manipulation Tools**: Removed `claim_ticket`, `release_ticket`, and `update_ticket_stage` tools to prevent pipeline stalls
- Tools that could bypass the automated queue system and create stalled states

### Changed
- **Streamlined Workflow**: All ticket processing now flows through automated queue system
- **Improved Stability**: Eliminates manual interventions that could disrupt worker coordination
- **Tool Count**: Evolved to 47 MCP tools with comprehensive feature coverage including bidirectional communication

### Security
- **Permission Modes**: Three distinct permission modes for different security requirements
  - Bypass mode: Development/testing with full access
  - Inherit mode: Production using project Claude Code permissions
  - File mode: Custom worker-specific permissions
- **Worker Isolation**: Proper permission enforcement for headless worker processes
- **Secure Defaults**: Default to file mode for explicit permission control

## Notes

This is the first public release of Vibe-Ensemble MCP. The system enables coordinated multi-agent development workflows with specialized AI workers handling different stages of complex projects.

For installation and usage instructions, see the [README.md](README.md).