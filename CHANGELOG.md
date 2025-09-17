# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] - 2025-01-17

### Added
- **🧠 Task Breakdown Sizing Methodology**: Intelligent task breakdown with optimal context-performance optimization (~120K token budget per stage)
- **📐 Natural Boundary Detection**: Automatic task splitting along technology, functional, and expertise boundaries
- **⚡ Enhanced Planning Workers**: Built-in token estimation and pipeline optimization with comprehensive validation
- **📊 Real-Time SSE Integration**: Full Server-Sent Events protocol for live progress monitoring and event streaming
- **🔧 Improved Worker Templates**: 8 specialized worker templates with task sizing methodology integration
- **📋 Enhanced Coordinator Prompts**: Updated coordination with systematic task delegation and sizing guidance
- **🛠️ Robust MCP Tools**: 19 MCP tools with enhanced project metadata and worker coordination
- **📚 Comprehensive Documentation**: Complete SSE protocol implementation and task breakdown sizing methodology
- **🔒 Enhanced Security**: Permission system with three modes (bypass, inherit, file) for fine-grained worker access control

### Removed
- **Manual Ticket Manipulation Tools**: Removed `claim_ticket`, `release_ticket`, and `update_ticket_stage` tools to prevent pipeline stalls
- Tools that could bypass the automated queue system and create stalled states

### Changed
- **Streamlined Workflow**: All ticket processing now flows through automated queue system
- **Improved Stability**: Eliminates manual interventions that could disrupt worker coordination
- **Tool Count**: Reduced from 22 to 19 MCP tools by removing redundant manual manipulation tools

### Security
- **Permission Modes**: Three distinct permission modes for different security requirements
  - Bypass mode: Development/testing with full access
  - Inherit mode: Production using project Claude Code permissions
  - File mode: Custom worker-specific permissions
- **Worker Isolation**: Proper permission enforcement for headless worker processes
- **Secure Defaults**: Default to inherit mode for balanced security and functionality

## Notes

This is the first public release of Vibe-Ensemble MCP. The system enables coordinated multi-agent development workflows with specialized AI workers handling different stages of complex projects.

For installation and usage instructions, see the [README.md](README.md).