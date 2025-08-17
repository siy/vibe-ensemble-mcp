-- Initial schema for Vibe Ensemble MCP Server

-- Agents table
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL,
    capabilities TEXT NOT NULL,
    status TEXT NOT NULL,
    connection_metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen TEXT NOT NULL
);

-- Issues table  
CREATE TABLE IF NOT EXISTS issues (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    assigned_agent_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    resolved_at TEXT,
    tags TEXT NOT NULL,
    FOREIGN KEY (assigned_agent_id) REFERENCES agents(id)
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL,
    recipient_id TEXT,
    message_type TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    delivered_at TEXT,
    FOREIGN KEY (sender_id) REFERENCES agents(id),
    FOREIGN KEY (recipient_id) REFERENCES agents(id)
);

-- Knowledge table
CREATE TABLE IF NOT EXISTS knowledge (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    knowledge_type TEXT NOT NULL,
    tags TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL,
    access_level TEXT NOT NULL,
    FOREIGN KEY (created_by) REFERENCES agents(id)
);

-- System prompts table
CREATE TABLE IF NOT EXISTS system_prompts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    template TEXT NOT NULL,
    prompt_type TEXT NOT NULL,
    variables TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY (created_by) REFERENCES agents(id)
);

-- Agent templates table
CREATE TABLE IF NOT EXISTS agent_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    agent_type TEXT NOT NULL,
    capabilities TEXT NOT NULL,
    system_prompt_id TEXT,
    workflow_steps TEXT NOT NULL,
    configuration_params TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY (created_by) REFERENCES agents(id),
    FOREIGN KEY (system_prompt_id) REFERENCES system_prompts(id)
);

-- Indexes for better performance
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
CREATE INDEX IF NOT EXISTS idx_agents_type ON agents(agent_type);
CREATE INDEX IF NOT EXISTS idx_agents_last_seen ON agents(last_seen);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_issues_priority ON issues(priority);
CREATE INDEX IF NOT EXISTS idx_issues_assigned ON issues(assigned_agent_id);
CREATE INDEX IF NOT EXISTS idx_issues_created ON issues(created_at);
CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_id);
CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(recipient_id);
CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at);
CREATE INDEX IF NOT EXISTS idx_messages_type ON messages(message_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_type ON knowledge(knowledge_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_created_by ON knowledge(created_by);
CREATE INDEX IF NOT EXISTS idx_knowledge_access_level ON knowledge(access_level);
CREATE INDEX IF NOT EXISTS idx_knowledge_created ON knowledge(created_at);
CREATE INDEX IF NOT EXISTS idx_prompts_type ON system_prompts(prompt_type);
CREATE INDEX IF NOT EXISTS idx_prompts_active ON system_prompts(is_active);
CREATE INDEX IF NOT EXISTS idx_prompts_name ON system_prompts(name);
CREATE INDEX IF NOT EXISTS idx_templates_type ON agent_templates(agent_type);
CREATE INDEX IF NOT EXISTS idx_templates_active ON agent_templates(is_active);
CREATE INDEX IF NOT EXISTS idx_templates_name ON agent_templates(name);
CREATE INDEX IF NOT EXISTS idx_templates_prompt ON agent_templates(system_prompt_id);
CREATE INDEX IF NOT EXISTS idx_templates_created_by ON agent_templates(created_by);