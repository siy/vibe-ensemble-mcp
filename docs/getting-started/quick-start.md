# Quick Start Guide

Get Vibe Ensemble running and coordinating your first Claude Code agents in under 5 minutes.

## What You'll Need

- A computer running macOS, Linux, or Windows
- Claude Code (install from [claude.ai/code](https://claude.ai/code))
- 5 minutes to set up

## Step 1: Install Vibe Ensemble

**macOS/Linux:**
```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash
```

**Windows:**
```powershell
iwr https://vibeensemble.dev/install.ps1 -UseBasicParsing | iex
```

Verify the installation:
```bash
vibe-ensemble --version
```

## Step 2: Start the Server

```bash
vibe-ensemble
```

You should see:
```
🚀 Vibe Ensemble started successfully
📊 Web dashboard: http://127.0.0.1:8080
💾 Database: ~/.vibe-ensemble/data.db
```

Leave this running in a terminal - it's your coordination server.

## Step 3: Connect Claude Code

Add Vibe Ensemble as an MCP server in Claude Code:

### Option A: Settings UI
1. Open Claude Code
2. Go to Settings (Cmd/Ctrl + ,)
3. Navigate to "MCP Servers"
4. Click "Add Server"
5. Enter:
   - **Name**: `vibe-ensemble`
   - **Command**: `vibe-ensemble --mcp-only --transport=stdio`

### Option B: Configuration File
Add this to your Claude Code MCP settings file:

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

## Step 4: Test the Connection

In Claude Code, try using a coordination tool:

```
Can you register this instance as a frontend development agent?
```

Claude Code should now be able to use tools like:
- `vibe/agent/register` - Register as an agent
- `vibe/agent/list` - See all connected agents
- `vibe/issue/create` - Create shared tasks

## Step 5: Your First Multi-Agent Setup

Now let's set up multiple agents working together:

1. **Keep Vibe Ensemble running** in Terminal 1

2. **Start Agent 1** (Frontend) in Terminal 2:
   ```bash
   cd ~/your-project
   claude-code
   ```
   Then tell it: "Register as a frontend development agent specializing in React and TypeScript"

3. **Start Agent 2** (Backend) in Terminal 3:
   ```bash
   cd ~/your-api-project  
   claude-code
   ```
   Then tell it: "Register as a backend development agent specializing in Node.js and databases"

4. **Watch them coordinate** - Agent 1 can now:
   - Create issues that Agent 2 can see
   - Share knowledge about API contracts
   - Avoid conflicts when both work on the same codebase

## Step 6: Monitor the Coordination

Open the web dashboard: http://127.0.0.1:8080

You'll see:
- **Overview**: Active agents and recent activity
- **Agents**: Details about each connected agent
- **Issues**: Shared tasks and coordination
- **Knowledge**: Insights shared between agents

## Common First Use Cases

### Single Project, Multiple Specializations
- **Code Writer**: Implements features
- **Code Reviewer**: Reviews PRs and suggests improvements
- **Test Writer**: Adds and maintains tests
- **Doc Writer**: Keeps documentation updated

### Multiple Projects
- **Project A Agent**: Works on your web app
- **Project B Agent**: Works on your mobile app  
- **Shared Agent**: Handles cross-project coordination

### Team Collaboration
- Each team member runs their own agents
- All agents coordinate through the same Vibe Ensemble
- Shared knowledge base and issue tracking

## Quick Troubleshooting

**Server won't start:**
```bash
# Check if port 8080 is in use
lsof -i :8080  # macOS/Linux
netstat -ano | findstr :8080  # Windows

# Use different port if needed
vibe-ensemble --port=8081
```

**Claude Code can't connect:**
1. Make sure Vibe Ensemble is running
2. Check the MCP configuration is correct
3. Restart Claude Code after adding the server

**Tools not working:**
- Verify in Claude Code: "Can you list available vibe tools?"
- Check the web dashboard for agent connections

## What's Next?

- **Explore the web dashboard** to understand how agents coordinate
- **Try different agent specializations** for your workflow
- **Create shared issues** to coordinate complex tasks
- **Share knowledge** between agents to avoid repeating work
- **Read the [User Guide](../user-guide.md)** for advanced workflows

## Getting Help

- **Web Dashboard**: Check http://127.0.0.1:8080 for system status
- **GitHub Issues**: [Report problems](https://github.com/siy/vibe-ensemble-mcp/issues)
- **Discussions**: [Ask questions](https://github.com/siy/vibe-ensemble-mcp/discussions)

You now have a local AI agent coordination system running! Your Claude Code instances can work together without conflicts, share knowledge, and coordinate on complex tasks.