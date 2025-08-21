# Web Interface Guide

The Vibe Ensemble MCP Server includes a comprehensive web interface for managing agents, issues, knowledge, and monitoring system activity. This guide covers all aspects of using the web interface effectively.

## Getting Started

### Accessing the Web Interface

The web interface is available at the server's base URL:
- **Development**: `http://localhost:8080`
- **Production**: `https://your-domain.com`

### Authentication

#### Login Process
1. Navigate to the web interface URL
2. Click "Login" or you'll be automatically redirected to the login page
3. Enter your credentials
4. Click "Sign In"

#### User Roles
- **Administrator**: Full access to all features and system management
- **Coordinator**: Agent management and issue coordination capabilities  
- **Viewer**: Read-only access to dashboard and monitoring information

#### Password Requirements
- Minimum 8 characters
- Must include uppercase and lowercase letters
- Must include at least one number
- Special characters recommended

## Dashboard Overview

### Main Dashboard

The dashboard provides a comprehensive overview of your Vibe Ensemble system:

#### System Statistics
- **Active Agents**: Number of currently connected and active agents
- **Total Issues**: Count of all issues in the system
- **Messages Exchanged**: Total inter-agent communications
- **Knowledge Entries**: Number of stored knowledge items

#### Recent Activity
- Real-time system status updates
- Service health indicators
- Recent system events and notifications

#### Quick Actions
- **Create New Issue**: Directly create and assign new issues
- **Add Knowledge**: Add new knowledge entries to the repository
- **View Agents**: Access the agent management interface
- **API Statistics**: View detailed API usage metrics

### System Overview Cards

#### Agent Management
- Agent registration and lifecycle tracking
- Capability management and monitoring
- Real-time status updates

#### Issue Tracking  
- Persistent task and problem management
- Priority-based workflow organization
- Agent assignment and progress tracking

#### Knowledge Repository
- Development patterns and best practices storage
- Searchable content with categorization
- Version control and collaboration features

### Recent Issues Table

Displays the most recently created or updated issues:
- **Title**: Brief description of the issue
- **Priority**: Low, Medium, High, or Critical priority level
- **Status**: Open, InProgress, Resolved, or Closed
- **Created**: Timestamp of issue creation
- **Actions**: Quick access to view or edit issues

## Agent Management

### Agent List View

Navigate to `/agents` to view all registered agents:

#### Agent Information Display
- **Agent ID**: Unique identifier for the agent
- **Name**: Human-readable agent name
- **Type**: Coordinator, Worker, or Monitor
- **Status**: Active, Inactive, Disconnected, or Error
- **Capabilities**: List of agent's declared capabilities
- **Last Seen**: Timestamp of last activity

#### Agent Status Indicators
- 🟢 **Active**: Agent is connected and operational
- 🟡 **Inactive**: Agent is registered but not currently active
- 🔴 **Disconnected**: Agent has lost connection
- ⚠️ **Error**: Agent is experiencing errors

### Agent Details

Click on any agent to view detailed information:

#### Basic Information
- Agent registration timestamp
- Last activity and connection status  
- Configuration details and metadata

#### Capabilities
- Declared agent capabilities
- Supported task types
- Resource requirements

#### Activity History
- Recent messages sent and received
- Task assignments and completions
- Status change history

#### Performance Metrics
- Task completion rate
- Average response time
- Error rate and reliability metrics

### Agent Actions

#### From Agent List
- **View Details**: Access comprehensive agent information
- **Send Message**: Initiate direct communication with the agent
- **Assign Task**: Assign specific issues or tasks
- **Update Status**: Manually update agent status if needed

#### From Agent Details
- **Edit Configuration**: Modify agent settings (admin only)
- **Deregister**: Remove agent from system (admin only)
- **View Messages**: See all communications with the agent
- **Export Data**: Download agent data and metrics

## Issue Management

### Issue List View

Access the issue management interface at `/issues`:

#### Issue Display Options
- **List View**: Tabular display with sorting and filtering
- **Kanban View**: Visual workflow board (if enabled)
- **Calendar View**: Timeline-based issue tracking

#### Filtering and Search
- **Status Filter**: Open, InProgress, Resolved, Closed
- **Priority Filter**: Low, Medium, High, Critical
- **Assignee Filter**: Filter by assigned agent
- **Date Range**: Filter by creation or update dates
- **Search**: Full-text search across titles and descriptions

#### Sorting Options
- **Created Date**: Newest or oldest first
- **Priority**: Highest to lowest or reverse
- **Status**: Group by current status
- **Assigned Agent**: Group by agent assignments

### Creating Issues

#### New Issue Form
1. Navigate to `/issues/new` or click "Create New Issue"
2. Fill in the required information:
   - **Title**: Concise issue description
   - **Description**: Detailed problem statement
   - **Priority**: Select appropriate priority level
   - **Assigned Agent**: Choose from available agents (optional)
   - **Labels**: Add relevant tags for categorization

#### Issue Creation Best Practices
- Write clear, descriptive titles
- Include reproduction steps for bugs
- Specify acceptance criteria for features
- Add relevant labels for better organization
- Assign to appropriate agent when possible

### Issue Details View

#### Issue Information
- **Title and Description**: Full issue details
- **Status Tracking**: Current status with history
- **Priority Level**: Visual priority indicator
- **Assignment Information**: Assigned agent details
- **Timestamps**: Creation, update, and resolution dates

#### Issue Actions
- **Edit Issue**: Modify title, description, or priority
- **Change Status**: Update issue status
- **Reassign**: Change assigned agent
- **Add Comment**: Include additional information
- **Close Issue**: Mark as resolved or closed

#### Activity Timeline
- Status changes with timestamps
- Agent assignments and reassignments
- Comments and updates
- Related message activity

### Issue Workflows

#### Standard Issue Lifecycle
1. **Open**: Issue is created and waiting for assignment
2. **InProgress**: Agent is actively working on the issue
3. **Resolved**: Issue has been fixed or completed
4. **Closed**: Issue is verified complete and archived

#### Priority-Based Processing
- **Critical**: Immediate attention required
- **High**: Important issues requiring prompt action
- **Medium**: Standard priority with normal processing
- **Low**: Non-urgent issues processed when time allows

## Knowledge Management

### Knowledge Repository

Access the knowledge base at `/knowledge`:

#### Knowledge Categories
- **Patterns**: Reusable development patterns
- **Practices**: Established methodologies and procedures
- **Guidelines**: Organizational standards and policies
- **Solutions**: Problem-solving approaches and fixes

#### Search and Discovery
- **Full-Text Search**: Search across all knowledge content
- **Category Filtering**: Browse by knowledge category
- **Tag-Based Filtering**: Find content by associated tags
- **Recent Additions**: View newly added knowledge entries

### Viewing Knowledge Entries

#### Knowledge Entry Display
- **Title and Content**: Full knowledge article
- **Category and Tags**: Classification information
- **Author Information**: Who contributed the knowledge
- **Version History**: Track changes over time
- **Related Entries**: Linked or similar content

#### Knowledge Actions
- **Edit Entry**: Update content (if authorized)
- **Add Tags**: Improve categorization
- **Share**: Generate shareable links
- **Export**: Download in various formats
- **Report Issue**: Flag problems with content

### Contributing Knowledge

#### Creating Knowledge Entries
1. Navigate to `/knowledge/new`
2. Fill in the knowledge form:
   - **Title**: Clear, descriptive title
   - **Content**: Comprehensive knowledge content
   - **Category**: Select appropriate category
   - **Tags**: Add relevant tags
   - **Visibility**: Set access permissions

#### Knowledge Best Practices
- Write clear, actionable content
- Include examples and code snippets
- Use proper formatting and structure
- Add relevant tags for discoverability
- Reference related issues or patterns

## Real-time Features

### WebSocket Notifications

The web interface uses WebSocket connections for real-time updates:

#### Automatic Updates
- **Agent Status Changes**: Real-time agent connectivity updates
- **Issue Status Updates**: Live issue status and assignment changes
- **New Messages**: Instant notification of agent communications
- **System Alerts**: Important system notifications

#### Visual Indicators
- 🔴 **Disconnected**: No real-time connection
- 🟡 **Connecting**: Establishing WebSocket connection
- 🟢 **Connected**: Real-time updates active

### Live Data Refresh

#### Auto-Refresh Components
- Dashboard statistics update every 30 seconds
- Agent status indicators refresh automatically
- Issue lists update when changes occur
- Knowledge search results refresh on new additions

#### Manual Refresh Options
- Refresh button on each major page
- Browser refresh maintains current view and filters
- Force refresh option for troubleshooting

## Customization and Preferences

### User Settings

Access user preferences through the profile menu:

#### Display Preferences
- **Theme**: Light or dark mode selection
- **Language**: Interface language selection
- **Time Zone**: Date/time display preferences
- **Items Per Page**: Pagination preferences

#### Notification Settings
- **Email Notifications**: Configure email alerts
- **Browser Notifications**: Enable desktop notifications
- **Notification Frequency**: Set notification intervals
- **Alert Priorities**: Choose which events trigger alerts

### Dashboard Customization

#### Widget Configuration
- Show/hide dashboard statistics
- Customize quick action buttons
- Configure recent activity display
- Set dashboard refresh intervals

#### Layout Options
- Grid layout preferences
- Card size and arrangement
- Default page preferences
- Navigation menu customization

## Mobile Interface

### Responsive Design

The web interface is optimized for mobile devices:

#### Mobile-Optimized Features
- Touch-friendly navigation and buttons
- Responsive layout for all screen sizes
- Simplified mobile menus
- Optimized forms for mobile input

#### Mobile Limitations
- Some advanced features require desktop browser
- File uploads work best on desktop
- Complex data views are simplified on mobile

## Keyboard Shortcuts

### Navigation Shortcuts
- `Ctrl+/`: Show help and shortcuts
- `Ctrl+D`: Go to dashboard
- `Ctrl+A`: Go to agents page
- `Ctrl+I`: Go to issues page
- `Ctrl+K`: Go to knowledge page

### Action Shortcuts
- `Ctrl+N`: Create new issue
- `Ctrl+S`: Save current form
- `Ctrl+R`: Refresh current page
- `Esc`: Close modal dialogs

## Troubleshooting

### Common Issues

#### Connection Problems
**Symptoms**: Real-time updates not working, stale data
**Solutions**:
- Check internet connection
- Refresh the page
- Clear browser cache
- Disable browser extensions temporarily

#### Authentication Issues
**Symptoms**: Login failures, session timeouts
**Solutions**:
- Verify username and password
- Check if account is active
- Clear browser cookies
- Contact administrator for password reset

#### Performance Issues
**Symptoms**: Slow page loads, unresponsive interface
**Solutions**:
- Check network connection speed
- Close unnecessary browser tabs
- Disable browser extensions
- Use latest browser version

### Browser Compatibility

#### Supported Browsers
- **Chrome**: Version 90+
- **Firefox**: Version 88+
- **Safari**: Version 14+
- **Edge**: Version 90+

#### Required Features
- JavaScript enabled
- WebSocket support
- Local storage support
- Modern CSS support

### Getting Help

#### In-Interface Help
- Hover tooltips on form fields
- Help buttons on complex features
- Inline documentation links
- Error message guidance

#### External Resources
- [API Documentation](../api/overview.md)
- [Troubleshooting Guide](../troubleshooting/common-issues.md)
- [FAQ](../troubleshooting/faq.md)
- [Community Forum](https://github.com/siy/vibe-ensemble-mcp/discussions)

---

*For additional web interface features and advanced usage patterns, see the [Workflows Guide](workflows.md) and [Advanced Features](../advanced/web-features.md).*