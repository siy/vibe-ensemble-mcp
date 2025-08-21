# Basic Usage Examples

This guide provides practical examples of using the Vibe Ensemble MCP Server for common scenarios. Each example includes step-by-step instructions and expected outcomes.

## Quick Start Example

### Setting Up Your First Agent Team

This example shows how to set up a basic coordinator-worker team for a simple development project.

#### Step 1: Start the Server

```bash
# Start the server with Docker
docker run -d \
  --name vibe-ensemble \
  -p 8080:8080 \
  -e DATABASE_URL="sqlite:///data/vibe-ensemble.db" \
  -e JWT_SECRET="development-secret-key-32-chars" \
  -v vibe-data:/data \
  vibe-ensemble:latest

# Verify server is running
curl http://localhost:8080/api/health
```

**Expected Response**:
```json
{
  "status": "healthy",
  "timestamp": "2025-08-19T10:00:00Z"
}
```

#### Step 2: Access Web Interface

1. Open browser to `http://localhost:8080`
2. Login with default credentials (admin/admin)
3. Complete initial setup wizard
4. Set secure password and system preferences

#### Step 3: Configure Claude Code Agents

**Coordinator Agent**:
```bash
# Configure coordinator agent
claude-code config set mcp.server_url "http://localhost:8080"
claude-code config set agent.name "project-coordinator"
claude-code config set agent.type "Coordinator"
claude-code config set agent.capabilities "planning,coordination,communication"

# Start coordinator
claude-code --agent-mode coordinator
```

**Worker Agent**:
```bash
# Configure worker agent  
claude-code config set mcp.server_url "http://localhost:8080"
claude-code config set agent.name "dev-worker-1"
claude-code config set agent.type "Worker"
claude-code config set agent.capabilities "coding,testing,debugging"

# Start worker
claude-code --agent-mode worker
```

#### Step 4: Verify Agent Registration

Check the web interface at `http://localhost:8080/agents`:
- Both agents should appear with "Active" status
- Coordinator shows planning and coordination capabilities
- Worker shows coding and testing capabilities

## Simple Issue Management

### Creating and Assigning Issues

#### Example 1: Bug Report via Web Interface

1. **Navigate to Issues**: Click "Issues" in the navigation menu
2. **Create New Issue**: Click "Create New Issue" button
3. **Fill Form**:
   ```
   Title: Fix login timeout issue
   Description: Users are experiencing session timeouts after 5 minutes of inactivity
   Priority: High
   Assigned Agent: dev-worker-1
   Labels: bug, authentication, urgent
   ```
4. **Submit**: Click "Create Issue"

**Expected Result**: Issue appears in issue list with "Open" status and is assigned to the specified worker agent.

#### Example 2: Feature Request via API

```bash
# Create feature request via API
curl -X POST http://localhost:8080/api/issues \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{
    "title": "Add user profile pictures",
    "description": "Allow users to upload and display profile pictures in the application",
    "priority": "Medium",
    "assigned_agent_id": null
  }'
```

**Expected Response**:
```json
{
  "issue": {
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "title": "Add user profile pictures",
    "status": "Open",
    "priority": "Medium",
    "created_at": "2025-08-19T10:30:00Z"
  },
  "timestamp": "2025-08-19T10:30:00Z"
}
```

### Issue Resolution Workflow

#### Example: Bug Fix Process

1. **Agent Receives Assignment**:
   - Worker agent gets notification of new assigned issue
   - Agent updates issue status to "InProgress"
   - Agent begins investigation

2. **Progress Updates**:
   ```bash
   # Agent updates issue via API
   curl -X PUT http://localhost:8080/api/issues/123e4567-e89b-12d3-a456-426614174000 \
     -H "Authorization: Bearer $AGENT_TOKEN" \
     -d '{
       "status": "InProgress",
       "progress_notes": "Identified issue in session timeout configuration"
     }'
   ```

3. **Resolution**:
   - Agent implements fix
   - Updates issue with resolution details
   - Changes status to "Resolved"

4. **Verification**:
   - Coordinator or human verifies fix
   - Issue status changed to "Closed"

## Knowledge Management Examples

### Creating Knowledge Entries

#### Example 1: Best Practice Documentation

```bash
# Create knowledge entry via API
curl -X POST http://localhost:8080/api/knowledge \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Database Connection Pooling Best Practices",
    "content": "# Database Connection Pooling\n\nWhen implementing connection pooling:\n\n1. **Pool Sizing**: Start with connections = CPU cores\n2. **Timeouts**: Set reasonable connection timeouts\n3. **Monitoring**: Track pool usage and performance\n\n## Example Configuration\n\n```rust\nlet pool = PgPoolOptions::new()\n    .max_connections(10)\n    .connect_timeout(Duration::from_secs(5))\n    .connect(&database_url)\n    .await?;\n```\n\nThis ensures optimal database performance while preventing connection exhaustion.",
    "category": "database",
    "tags": ["database", "performance", "connection-pooling", "rust"]
  }'
```

#### Example 2: Problem-Solution Pattern

**Via Web Interface**:
1. Navigate to Knowledge → "Add New Entry"
2. Fill in the form:
   ```
   Title: Fixing Memory Leaks in Async Applications
   Category: debugging
   Tags: memory, async, rust, debugging
   Content: [Detailed explanation with code examples]
   ```

### Knowledge Search and Discovery

#### Example: Finding Relevant Knowledge

```bash
# Search for database-related knowledge
curl "http://localhost:8080/api/knowledge?search=database&category=performance"

# Get specific knowledge entry
curl "http://localhost:8080/api/knowledge/abc12345-e89b-12d3-a456-426614174003"
```

**Search Results**:
```json
{
  "knowledge": [
    {
      "id": "abc12345-e89b-12d3-a456-426614174003",
      "title": "Database Connection Pooling Best Practices",
      "category": "database",
      "tags": ["database", "performance", "connection-pooling"],
      "created_at": "2025-08-19T09:00:00Z"
    }
  ],
  "total": 1
}
```

## Agent Communication Examples

### Direct Agent Messaging

#### Example: Coordinator Assigning Tasks

```json
{
  "type": "task_assignment",
  "from_agent": "project-coordinator",
  "to_agent": "dev-worker-1",
  "content": {
    "task_id": "123e4567-e89b-12d3-a456-426614174000",
    "task_type": "bug_fix",
    "priority": "high",
    "description": "Fix authentication timeout issue",
    "deadline": "2025-08-20T17:00:00Z",
    "requirements": [
      "Investigate session timeout configuration",
      "Implement fix with backward compatibility",
      "Add tests to prevent regression",
      "Update documentation if needed"
    ]
  },
  "metadata": {
    "correlation_id": "task-123",
    "requires_confirmation": true
  }
}
```

#### Example: Worker Status Update

```json
{
  "type": "status_update",
  "from_agent": "dev-worker-1",
  "to_agent": "project-coordinator",
  "content": {
    "task_id": "123e4567-e89b-12d3-a456-426614174000",
    "status": "in_progress",
    "progress_percentage": 60,
    "summary": "Issue identified and fix implemented",
    "details": {
      "completed_steps": [
        "Analyzed session timeout configuration",
        "Identified root cause in JWT expiration handling",
        "Implemented configurable timeout values"
      ],
      "next_steps": [
        "Add comprehensive tests",
        "Update configuration documentation"
      ],
      "estimated_completion": "2025-08-19T16:00:00Z"
    }
  }
}
```

### Broadcast Messages

#### Example: System-wide Announcements

```json
{
  "type": "system_announcement",
  "from_agent": "project-coordinator",
  "to_agent": null,
  "content": {
    "announcement_type": "maintenance_window",
    "title": "Scheduled Database Maintenance",
    "message": "Database maintenance scheduled for tonight 2 AM - 4 AM UTC. All agents should complete current tasks and prepare for temporary service interruption.",
    "scheduled_time": "2025-08-20T02:00:00Z",
    "duration_hours": 2,
    "impact": "Database operations will be unavailable during maintenance window"
  },
  "metadata": {
    "priority": "high",
    "requires_acknowledgment": true
  }
}
```

## Real-time Features

### WebSocket Notifications

#### Example: Monitoring Live Updates

**JavaScript Client**:
```javascript
// Connect to WebSocket for real-time updates
const ws = new WebSocket('ws://localhost:8080/ws');

ws.onopen = function() {
    console.log('Connected to Vibe Ensemble WebSocket');
};

ws.onmessage = function(event) {
    const data = JSON.parse(event.data);
    console.log('Received update:', data);
    
    switch(data.type) {
        case 'agent_status_changed':
            updateAgentStatus(data.agent_id, data.status);
            break;
        case 'issue_created':
            addIssueToList(data.issue);
            break;
        case 'issue_status_changed':
            updateIssueStatus(data.issue_id, data.status);
            break;
        case 'message_received':
            displayMessage(data.message);
            break;
    }
};

function updateAgentStatus(agentId, status) {
    const statusElement = document.getElementById(`agent-${agentId}-status`);
    statusElement.textContent = status;
    statusElement.className = `status-${status.toLowerCase()}`;
}
```

## API Integration Examples

### External Tool Integration

#### Example: Monitoring System Integration

**Alerting System** → **Vibe Ensemble** → **Agent Response**:

```bash
#!/bin/bash
# monitoring-alert.sh - Called by monitoring system when issue detected

ALERT_MESSAGE="$1"
PRIORITY="$2"
COMPONENT="$3"

# Create issue in Vibe Ensemble
ISSUE_ID=$(curl -s -X POST http://localhost:8080/api/issues \
  -H "Authorization: Bearer $MONITORING_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"title\": \"Alert: $COMPONENT Issue Detected\",
    \"description\": \"$ALERT_MESSAGE\",
    \"priority\": \"$PRIORITY\",
    \"labels\": [\"monitoring\", \"alert\", \"$COMPONENT\"]
  }" | jq -r '.issue.id')

echo "Created issue: $ISSUE_ID"

# Notify relevant agents
curl -X POST http://localhost:8080/api/messages \
  -H "Authorization: Bearer $MONITORING_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"to_agent\": null,
    \"message_type\": \"alert\",
    \"content\": {
      \"alert_type\": \"system_alert\",
      \"component\": \"$COMPONENT\",
      \"severity\": \"$PRIORITY\",
      \"issue_id\": \"$ISSUE_ID\",
      \"message\": \"$ALERT_MESSAGE\"
    }
  }"
```

#### Example: CI/CD Pipeline Integration

**GitHub Actions Workflow**:
```yaml
name: Report Build Failure
on:
  workflow_run:
    workflows: ["CI"]
    types: [completed]
    
jobs:
  report-failure:
    if: ${{ github.event.workflow_run.conclusion == 'failure' }}
    runs-on: ubuntu-latest
    steps:
      - name: Create Issue for Build Failure
        run: |
          curl -X POST ${{ secrets.VIBE_ENSEMBLE_URL }}/api/issues \
            -H "Authorization: Bearer ${{ secrets.VIBE_ENSEMBLE_TOKEN }}" \
            -H "Content-Type: application/json" \
            -d '{
              "title": "Build Failed: ${{ github.event.workflow_run.head_branch }}",
              "description": "CI pipeline failed for commit ${{ github.event.workflow_run.head_sha }}\n\nWorkflow: ${{ github.event.workflow_run.html_url }}",
              "priority": "High",
              "labels": ["ci", "build-failure", "automated"]
            }'
```

### Data Export and Reporting

#### Example: Generate Weekly Report

```python
#!/usr/bin/env python3
# weekly-report.py - Generate weekly team performance report

import requests
import json
from datetime import datetime, timedelta

# Configuration
BASE_URL = "http://localhost:8080"
TOKEN = "your-api-token-here"
HEADERS = {"Authorization": f"Bearer {TOKEN}"}

def get_weekly_stats():
    # Get issues created in last week
    week_ago = datetime.now() - timedelta(days=7)
    
    response = requests.get(f"{BASE_URL}/api/issues", headers=HEADERS)
    issues = response.json()['issues']
    
    # Filter issues from last week
    weekly_issues = [
        issue for issue in issues 
        if datetime.fromisoformat(issue['created_at'].replace('Z', '+00:00')) > week_ago
    ]
    
    # Get agent statistics
    response = requests.get(f"{BASE_URL}/api/agents", headers=HEADERS)
    agents = response.json()['agents']
    
    return {
        'total_issues': len(weekly_issues),
        'resolved_issues': len([i for i in weekly_issues if i['status'] == 'Resolved']),
        'active_agents': len([a for a in agents if a['status'] == 'Active']),
        'issues_by_priority': {
            'Critical': len([i for i in weekly_issues if i['priority'] == 'Critical']),
            'High': len([i for i in weekly_issues if i['priority'] == 'High']),
            'Medium': len([i for i in weekly_issues if i['priority'] == 'Medium']),
            'Low': len([i for i in weekly_issues if i['priority'] == 'Low'])
        }
    }

def generate_report():
    stats = get_weekly_stats()
    
    report = f"""
# Weekly Vibe Ensemble Report
## Week of {datetime.now().strftime('%Y-%m-%d')}

### Summary
- **Total Issues Created**: {stats['total_issues']}
- **Issues Resolved**: {stats['resolved_issues']}
- **Active Agents**: {stats['active_agents']}
- **Resolution Rate**: {(stats['resolved_issues']/stats['total_issues']*100):.1f}%

### Issues by Priority
- Critical: {stats['issues_by_priority']['Critical']}
- High: {stats['issues_by_priority']['High']}
- Medium: {stats['issues_by_priority']['Medium']}
- Low: {stats['issues_by_priority']['Low']}

Generated on {datetime.now().isoformat()}
"""
    
    with open(f"weekly-report-{datetime.now().strftime('%Y-%m-%d')}.md", 'w') as f:
        f.write(report)
    
    print("Report generated successfully!")

if __name__ == "__main__":
    generate_report()
```

## Testing and Validation

### Health Check Scripts

#### Example: Comprehensive System Check

```bash
#!/bin/bash
# health-check.sh - Comprehensive system health validation

set -e

BASE_URL="http://localhost:8080"
ERRORS=0

echo "🔍 Vibe Ensemble Health Check"
echo "================================"

# Test 1: Basic connectivity
echo -n "Testing server connectivity... "
if curl -sf "$BASE_URL/api/health" > /dev/null; then
    echo "✅ OK"
else
    echo "❌ FAILED"
    ((ERRORS++))
fi

# Test 2: Database connectivity
echo -n "Testing database connectivity... "
DB_RESPONSE=$(curl -s "$BASE_URL/api/stats" | jq -r '.timestamp' 2>/dev/null || echo "failed")
if [ "$DB_RESPONSE" != "failed" ] && [ "$DB_RESPONSE" != "null" ]; then
    echo "✅ OK"
else
    echo "❌ FAILED"
    ((ERRORS++))
fi

# Test 3: Agent connectivity (if token available)
if [ -n "$API_TOKEN" ]; then
    echo -n "Testing agent API... "
    AGENT_RESPONSE=$(curl -s -H "Authorization: Bearer $API_TOKEN" "$BASE_URL/api/agents" | jq -r '.timestamp' 2>/dev/null || echo "failed")
    if [ "$AGENT_RESPONSE" != "failed" ] && [ "$AGENT_RESPONSE" != "null" ]; then
        echo "✅ OK"
    else
        echo "❌ FAILED"
        ((ERRORS++))
    fi
fi

# Test 4: WebSocket connectivity
echo -n "Testing WebSocket... "
if command -v wscat >/dev/null 2>&1; then
    if timeout 5s wscat -c "ws://localhost:8080/ws" -x '{"type":"ping"}' >/dev/null 2>&1; then
        echo "✅ OK"
    else
        echo "❌ FAILED"
        ((ERRORS++))
    fi
else
    echo "⚠️  SKIPPED (wscat not installed)"
fi

echo "================================"
if [ $ERRORS -eq 0 ]; then
    echo "🎉 All tests passed!"
    exit 0
else
    echo "💥 $ERRORS test(s) failed!"
    exit 1
fi
```

## Next Steps

After trying these basic examples:

1. **Explore Advanced Features**: Try multi-agent coordination patterns
2. **Customize Configuration**: Adjust settings for your specific needs
3. **Integrate with Your Tools**: Connect to your existing development workflow
4. **Scale Up**: Add more agents and handle larger workloads
5. **Contribute**: Share your patterns and improvements with the community

For more complex scenarios, see:
- [Multi-Agent Coordination Examples](multi-agent.md)
- [Integration Examples](integrations.md)
- [Advanced Use Cases](use-cases.md)

---

*These examples provide a foundation for using Vibe Ensemble effectively. Adapt them to your specific needs and workflows.*