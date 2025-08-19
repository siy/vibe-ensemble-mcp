# Frequently Asked Questions (FAQ)

This document answers common questions about the Vibe Ensemble MCP Server, covering installation, configuration, usage, and troubleshooting.

## General Questions

### What is Vibe Ensemble MCP Server?

**Q: What exactly does the Vibe Ensemble MCP Server do?**

A: Vibe Ensemble is a coordination hub for multiple Claude Code instances that enables:
- **Distributed Task Execution**: Coordinate work across multiple AI agents
- **Unified Management**: Centralized control and monitoring of your agent ecosystem
- **Real-time Communication**: Seamless messaging between coordinator and worker agents
- **Issue Tracking**: Persistent task and problem management with web interface
- **Knowledge Management**: Organizational patterns, practices, and guidelines repository

### How is it different from using Claude Code directly?

**Q: Why use Vibe Ensemble instead of just Claude Code by itself?**

A: While Claude Code is excellent for individual tasks, Vibe Ensemble adds team coordination capabilities:
- **Scale Beyond One Agent**: Manage multiple Claude Code agents working in parallel
- **Persistent State**: Issues, knowledge, and progress persist across agent sessions
- **Knowledge Sharing**: Agents can share learnings and patterns with each other
- **Workflow Management**: Structured processes for complex, multi-step projects
- **Human Oversight**: Web interface for monitoring and directing agent activities

## Installation and Setup

### System Requirements

**Q: What are the minimum system requirements?**

A: Minimum requirements:
- **CPU**: 2 cores
- **Memory**: 4GB RAM  
- **Storage**: 20GB available space
- **OS**: Linux (Ubuntu 20.04+, CentOS 8+, RHEL 8+)
- **Network**: Outbound internet access

For production workloads, we recommend 4-8 cores and 8-16GB RAM.

**Q: Can I run this on Windows or macOS?**

A: While the server is primarily designed for Linux, you can run it on:
- **Windows**: Using WSL2 (Windows Subsystem for Linux)
- **macOS**: Natively supported for development
- **Docker**: Runs on any platform that supports Docker

### Database Selection

**Q: Should I use SQLite or PostgreSQL?**

A: Choose based on your deployment scale:
- **SQLite**: Perfect for development, testing, and small deployments (1-5 agents)
- **PostgreSQL**: Recommended for production, multiple agents, or high-throughput scenarios

SQLite is easier to set up, PostgreSQL offers better performance and scalability.

**Q: Can I migrate from SQLite to PostgreSQL later?**

A: Yes, migration tools are provided:
```bash
vibe-ensemble-server --export-data sqlite://old.db
vibe-ensemble-server --import-data postgresql://new-db-url
```

### Docker vs Native Installation

**Q: Should I use Docker or install natively?**

A: Both approaches have benefits:

**Docker (Recommended)**:
- Easier deployment and updates
- Consistent environment across different systems
- Simpler dependency management
- Better isolation and security

**Native Installation**:
- Better performance (no containerization overhead)
- Easier to debug and customize
- Direct access to system logs
- More control over system configuration

## Configuration and Security

### Security Best Practices

**Q: How do I secure my Vibe Ensemble deployment?**

A: Follow these security practices:

1. **Use Strong Secrets**:
   ```bash
   # Generate secure JWT secret (32+ characters)
   export JWT_SECRET="$(openssl rand -base64 32)"
   
   # Generate encryption key (exactly 32 characters)
   export ENCRYPTION_KEY="$(openssl rand -base64 32 | cut -c1-32)"
   ```

2. **Enable HTTPS**: Always use SSL/TLS in production
3. **Configure Firewall**: Only open required ports
4. **Regular Updates**: Keep system and dependencies updated
5. **Access Control**: Use strong passwords and limit user permissions

**Q: How often should I rotate JWT secrets?**

A: Rotate JWT secrets periodically for security:
- **Development**: Monthly
- **Production**: Every 3-6 months
- **After Incidents**: Immediately if security is compromised

Note that rotating JWT secrets will require all users to log in again.

### Performance Tuning

**Q: How do I optimize performance for many agents?**

A: Key performance optimizations:

1. **Database Tuning**:
   ```bash
   export DATABASE_POOL_SIZE="50"  # Increase for high concurrency
   ```

2. **Server Configuration**:
   ```bash
   export MAX_CONNECTIONS="2000"
   export WORKER_THREADS="8"  # Match CPU cores
   ```

3. **PostgreSQL Tuning**:
   ```sql
   ALTER SYSTEM SET max_connections = 200;
   ALTER SYSTEM SET shared_buffers = '1GB';
   ALTER SYSTEM SET effective_cache_size = '4GB';
   ```

4. **Use Caching**: Enable Redis for session storage (if available)

## Agent Management

### Agent Registration

**Q: How do agents register with the server?**

A: Agents register automatically when they first connect:
1. Configure Claude Code agent with server URL
2. Agent attempts to connect via MCP protocol
3. Server validates agent capabilities and assigns ID
4. Registration appears in web interface
5. Agent is ready to receive tasks

**Q: What if an agent fails to register?**

A: Common registration issues and solutions:
- **Network connectivity**: Check firewall and network settings
- **Authentication**: Verify server credentials and configuration
- **Protocol mismatch**: Ensure agent and server use compatible MCP versions
- **Server overload**: Check server capacity and performance

### Agent Types and Roles

**Q: What's the difference between Coordinator and Worker agents?**

A: Agent types serve different purposes:

**Coordinator Agents**:
- Act as team leads and planners
- Distribute tasks to worker agents
- Communicate with human users
- Make high-level decisions
- Typically one per team or project

**Worker Agents**:
- Execute specific tasks and assignments
- Report progress to coordinators
- Collaborate with other workers
- Focus on specialized capabilities
- Scale based on workload

### Agent Capabilities

**Q: How do I define what an agent can do?**

A: Capabilities are declared during agent configuration:
```bash
claude-code config set agent.capabilities "code-review,testing,debugging,documentation"
```

Common capabilities include:
- `code-review`: Code analysis and review
- `testing`: Test creation and execution
- `debugging`: Problem diagnosis and fixing
- `documentation`: Documentation writing
- `deployment`: Application deployment
- `monitoring`: System monitoring and alerting

## Issue Management

### Issue Workflow

**Q: What's the typical issue lifecycle?**

A: Standard issue workflow:
1. **Open**: Issue created, waiting for assignment
2. **InProgress**: Agent actively working on issue
3. **Resolved**: Issue completed, awaiting verification
4. **Closed**: Issue verified complete and archived

**Q: Can I customize the issue workflow?**

A: Currently the workflow is standardized, but you can:
- Use labels for additional categorization
- Add custom metadata for specific needs
- Create custom views and filters
- Use external tools that integrate via API

### Priority Management

**Q: How should I set issue priorities?**

A: Use priorities based on business impact:
- **Critical**: System down, security issues, data loss
- **High**: Major functionality broken, important deadlines
- **Medium**: Standard features, planned improvements
- **Low**: Nice-to-have features, cosmetic issues

**Q: Do agents automatically pick up high-priority issues?**

A: Priority handling depends on your configuration:
- **Manual Assignment**: You assign issues to specific agents
- **Automatic Assignment**: System assigns based on priority and agent availability
- **Agent Selection**: Agents can claim available high-priority issues

## Knowledge Management

### Knowledge Organization

**Q: How should I organize the knowledge repository?**

A: Effective knowledge organization strategies:

**By Category**:
- `patterns`: Reusable development patterns
- `practices`: Team processes and methodologies  
- `guidelines`: Coding standards and policies
- `solutions`: Problem-solving approaches

**By Technology**:
- `rust`, `python`, `javascript`: Language-specific knowledge
- `docker`, `kubernetes`: Infrastructure knowledge
- `database`, `api`: Domain-specific knowledge

**By Team/Project**:
- Use tags to associate knowledge with specific teams
- Project-specific documentation and lessons learned

### Knowledge Contribution

**Q: How do agents contribute knowledge?**

A: Agents can contribute knowledge in several ways:
1. **Automatic Extraction**: System identifies patterns from successful issue resolutions
2. **Manual Documentation**: Agents create knowledge entries via API
3. **Learning from Issues**: Successful solutions become reusable patterns
4. **Cross-project Sharing**: Agents share learnings across different projects

## API and Integration

### API Usage

**Q: How do I integrate with external tools?**

A: Use the REST API for integrations:
```bash
# Create issue from external system
curl -X POST https://your-server.com/api/issues \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "External system alert",
    "description": "Database performance degraded",
    "priority": "High"
  }'
```

Common integrations:
- **Monitoring systems**: Create issues from alerts
- **CI/CD pipelines**: Create issues from failed builds
- **Project management**: Sync with Jira, GitHub Issues, etc.
- **Communication tools**: Slack, Discord notifications

**Q: Is there a rate limit on API requests?**

A: Yes, default rate limits are:
- **Anonymous**: 1,000 requests/hour per IP
- **Authenticated**: 5,000 requests/hour per user
- **Admin users**: 10,000 requests/hour

Rate limits can be configured via environment variables.

### Authentication

**Q: How do I authenticate API requests?**

A: Use JWT Bearer tokens:
1. **Login**: POST to `/auth/login` with credentials
2. **Get Token**: Extract JWT token from response
3. **Use Token**: Include in Authorization header: `Bearer <token>`
4. **Refresh**: Use refresh token before expiration

**Q: How long do API tokens last?**

A: Default token lifetimes:
- **Access Token**: 24 hours
- **Refresh Token**: 7 days

These can be configured via `JWT_EXPIRY_HOURS` and `JWT_REFRESH_EXPIRY_HOURS`.

## Deployment and Operations

### Deployment Options

**Q: What's the best way to deploy in production?**

A: Recommended production deployment:
1. **Use Docker**: Easier management and updates
2. **PostgreSQL Database**: Better performance and reliability
3. **Reverse Proxy**: Nginx or similar for SSL termination
4. **Monitoring**: Set up health checks and alerting
5. **Backups**: Automated database and configuration backups

**Q: Can I run multiple server instances?**

A: Currently, Vibe Ensemble is designed as a single-instance application with a shared database. For high availability:
- Use database replication for failover
- Deploy behind a load balancer for redundancy
- Implement automated failover procedures

Horizontal scaling is planned for future releases.

### Monitoring and Maintenance

**Q: How do I monitor system health?**

A: Multiple monitoring approaches:

**Built-in Health Checks**:
```bash
curl http://localhost:8080/api/health
curl http://localhost:8080/api/stats
```

**Metrics Integration**:
- Prometheus metrics at `/metrics` endpoint
- Grafana dashboards for visualization
- Custom alerting rules for important events

**Log Monitoring**:
- Structured JSON logs for easy parsing
- Integration with log aggregation systems
- Automated log analysis and alerting

**Q: How often should I backup the system?**

A: Backup frequency recommendations:
- **Database**: Daily full backup, hourly incremental
- **Configuration**: Weekly or after changes
- **Knowledge Repository**: Daily (high value content)
- **Test Backups**: Monthly restore tests

## Troubleshooting

### Common Problems

**Q: Agents keep disconnecting, what should I check?**

A: Agent disconnection troubleshooting:
1. **Network Issues**: Check connectivity and firewalls
2. **Server Overload**: Monitor CPU and memory usage
3. **Configuration**: Verify timeout settings
4. **Agent Health**: Check agent logs for errors
5. **WebSocket Issues**: Test WebSocket connection directly

**Q: Web interface is slow, how can I improve performance?**

A: Performance optimization steps:
1. **Database**: Add indexes, optimize queries
2. **Server**: Increase worker threads, connection limits
3. **Browser**: Clear cache, disable extensions
4. **Network**: Check bandwidth and latency
5. **Caching**: Enable caching for static assets

### Getting Help

**Q: Where can I get help if I have issues?**

A: Multiple support channels available:
- **Documentation**: Comprehensive guides and references
- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Community questions and support
- **Troubleshooting Guide**: Systematic problem-solving steps

**Q: What information should I include when asking for help?**

A: Include these details:
- System information (OS, version, hardware)
- Configuration (sanitized, no secrets)
- Error messages and logs
- Steps to reproduce the issue
- What you've already tried

## Roadmap and Future Features

**Q: What features are planned for future releases?**

A: Upcoming features include:
- **Horizontal Scaling**: Multi-instance deployment support
- **Advanced Workflows**: Custom issue workflows and automation
- **Enhanced AI Features**: Smarter agent coordination and knowledge extraction
- **More Integrations**: Additional external tool integrations
- **Performance Improvements**: Better scalability and efficiency

**Q: How can I request new features?**

A: Feature request process:
1. **Check Existing Requests**: Search GitHub Issues
2. **Create Feature Request**: Use GitHub Issues template
3. **Provide Details**: Use case, requirements, benefits
4. **Community Discussion**: Engage with other users
5. **Development**: Features prioritized based on community needs

## Contributing

**Q: Can I contribute to the project?**

A: Yes! Contributions are welcome:
- **Code Contributions**: Bug fixes, new features
- **Documentation**: Improvements and translations
- **Testing**: Bug reports and testing help
- **Community**: Help other users, share experiences

See the [Contributing Guide](../developer/contributing.md) for details.

**Q: How do I report security issues?**

A: For security vulnerabilities:
- **Do NOT** create public issues
- Email security concerns to maintainers privately
- Include detailed description and reproduction steps
- Wait for confirmation before any public disclosure

---

*Have a question not answered here? Check our [GitHub Discussions](https://github.com/siy/vibe-ensemble-mcp/discussions) or create a new discussion.*