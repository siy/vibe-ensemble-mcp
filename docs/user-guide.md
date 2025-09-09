# User Guide

This guide will help you get the most out of Vibe Ensemble, from your first agent to coordinating complex multi-agent workflows.

## Getting Started

### Your First Agent

After [installing Vibe Ensemble](installation.md), let's set up your first coordinated Claude Code agent:

1. **Start Vibe Ensemble:**
   ```bash
   vibe-ensemble
   ```

2. **Connect Claude Code** by adding the MCP server to your configuration:
   ```json
   {
     "mcpServers": {
       "vibe-ensemble": {
         "command": "vibe-ensemble",
         "args": ["--mcp-only", "--transport=websocket", "--port=8081"],
         "transport": {
           "type": "websocket",
           "url": "ws://127.0.0.1:8081"
         }
       }
     }
   }
   ```

   **Alternative stdio transport** (legacy support):
   ```json
   {
     "mcpServers": {
       "vibe-ensemble": {
         "command": "vibe-ensemble --mcp-only --transport=stdio",
         "args": []
       }
     }
   }
   ```

3. **Open Claude Code** and you'll now have access to coordination tools like:
   - `vibe/agent/register` - Register this instance as an agent (deprecated, use web dashboard)
   - `vibe/agent/list` - See all connected agents (deprecated, use web dashboard)
   - `vibe/issue/create` - Create shared tasks (deprecated, use web dashboard)

### Multiple Agents Working Together

The real power comes when you have multiple Claude Code instances working together:

**Scenario: Frontend + Backend Development**

1. **Agent 1 (Frontend):**
   ```bash
   # In your React project directory
   claude-code  # Connected to Vibe Ensemble
   ```

2. **Agent 2 (Backend):**
   ```bash
   # In your API project directory  
   claude-code  # Connected to the same Vibe Ensemble
   ```

3. **Coordination in Action:**
   - Frontend agent creates an issue: "Need new API endpoint for user profiles"
   - Backend agent sees the issue and implements the endpoint
   - Both agents share knowledge about data structures and API contracts
   - No conflicting work or duplicate effort

## Web Dashboard

Open http://127.0.0.1:8080 to access the web dashboard:

### Overview Page
- **Active Agents**: Number of connected Claude Code instances
- **Open Issues**: Tasks that need attention
- **Recent Activity**: Latest coordination events
- **System Health**: Server status and performance

### Agents Page
- **Agent List**: All registered agents with their capabilities
- **Status**: Active, idle, or disconnected agents
- **Specializations**: What each agent is good at (frontend, backend, testing, etc.)
- **Current Tasks**: What each agent is currently working on

### Issues Page
- **Open Issues**: Tasks waiting for assignment or completion
- **In Progress**: Issues currently being worked on
- **Completed**: Recently finished tasks
- **Create Issue**: Button to add new coordination tasks

### Knowledge Base
- **Shared Insights**: Patterns and solutions discovered by agents
- **Search**: Find relevant knowledge for your current work
- **Categories**: Browse by topic (testing, deployment, debugging, etc.)
- **Add Knowledge**: Share your own discoveries

### System Monitoring
- **Performance Metrics**: CPU, memory, and database usage
- **Event Log**: Detailed coordination activity
- **Health Status**: System components status
- **Configuration**: Current settings and options

## Common Workflows

### Single Developer, Multiple Projects

**Setup:**
- One Vibe Ensemble instance running
- Different Claude Code agents for each project
- Shared knowledge base across all projects

**Benefits:**
- Learn from patterns across projects
- Avoid repeating solutions you've already found
- Track issues across your entire portfolio

**Example:**
```bash
# Terminal 1: Start coordination server
vibe-ensemble

# Terminal 2: Work on Project A
cd ~/projects/project-a
claude-code

# Terminal 3: Work on Project B  
cd ~/projects/project-b
claude-code
```

### Specialized Agent Roles

**Code Writer Agent:**
- Focused on implementing features
- Checks with other agents before making changes
- Shares implementation patterns

**Code Reviewer Agent:**
- Reviews PRs and suggests improvements
- Maintains code quality standards
- Shares review checklists and patterns

**Testing Agent:**
- Writes and maintains tests
- Ensures code coverage
- Shares testing strategies

**Documentation Agent:**
- Keeps documentation up to date
- Explains complex features
- Maintains README files and guides

### Team Coordination

**Small Team (2-5 developers):**
- Each developer runs their own Vibe Ensemble
- Agents can coordinate across different instances
- Shared knowledge base via git or shared storage

**Larger Team:**
- Central Vibe Ensemble server
- All agents connect to the same instance
- Team-wide coordination and knowledge sharing

## Best Practices

### Agent Registration
```bash
# Always register agents with clear roles
vibe/agent/register name="Frontend Dev" capabilities=["React", "TypeScript", "CSS"]
```

### Issue Management
```bash
# Create specific, actionable issues
vibe/issue/create title="Add dark mode toggle" priority="medium" type="feature"
```

### Knowledge Sharing
```bash
# Document patterns you discover
vibe/knowledge/add title="React State Management" content="Use Zustand for complex state..."
```

### Conflict Prevention
```bash
# Check for conflicts before major changes
vibe/conflict/detect file_path="src/api.ts" change_type="refactor"
```

## Advanced Features

### Custom Agent Specializations

You can create specialized agents for specific tasks:

**Database Agent:**
- Handles schema changes
- Optimizes queries  
- Manages migrations

**DevOps Agent:**
- Handles deployments
- Manages CI/CD pipelines
- Monitors production

**Security Agent:**
- Reviews code for security issues
- Audits dependencies
- Implements security best practices

### Cross-Project Coordination

For large codebases or microservices:

```bash
# Create project-wide issues
vibe/issue/create title="Update authentication library" 
  scope="all-projects" impact="breaking-change"

# Share knowledge across projects
vibe/knowledge/add title="JWT Implementation" 
  scope="backend-services" tags=["auth", "security"]
```

### Integration with External Tools

**Git Integration:**
- Agents can share branch status
- Coordinate merge conflicts
- Track deployment status

**Slack/Discord Integration:**
- Get notifications about important issues
- Share coordination updates with the team

## Troubleshooting

### Agent Not Connecting
1. Check that Vibe Ensemble is running: `curl http://127.0.0.1:8080/api/health`
2. Verify Claude Code MCP configuration
3. Check network connectivity and firewall settings

### Conflicts Not Being Detected
1. Ensure agents are properly registered
2. Check that file paths are consistent across agents
3. Verify agents are using the conflict detection tools

### Knowledge Not Being Shared
1. Check that knowledge entries have appropriate tags
2. Verify search functionality in web dashboard
3. Ensure agents are using knowledge tools actively

### Performance Issues
1. Monitor system resources in web dashboard
2. Check database size and optimize if needed
3. Consider increasing system resources

## Tips and Tricks

### Effective Agent Naming
- Use descriptive names: "Backend-UserService" vs "Agent1"
- Include technology stack: "Frontend-React-TypeScript"
- Specify project context: "ProjectA-Mobile-iOS"

### Issue Organization
- Use consistent tags and categories
- Set appropriate priorities
- Include detailed descriptions
- Update status regularly

### Knowledge Management
- Tag entries with relevant technologies
- Include code examples where helpful
- Update outdated information
- Create searchable titles

### Monitoring and Maintenance
- Check the dashboard daily for agent health
- Clean up completed issues regularly
- Archive old knowledge entries
- Update agent capabilities as they evolve

## Getting Help

If you need assistance:

1. **Check the web dashboard** for system status and recent activity
2. **Review logs** for error messages or unusual behavior
3. **Search existing issues** on GitHub for similar problems
4. **Create a new issue** with detailed information about your setup
5. **Join discussions** to connect with other users

## What's Next?

- Explore advanced coordination features
- Set up custom agent specializations
- Integrate with your existing development workflow
- Share your coordination patterns with the community

The more you use Vibe Ensemble, the better it becomes at helping your agents work together effectively. Start simple and gradually add more sophisticated coordination as you become comfortable with the system.