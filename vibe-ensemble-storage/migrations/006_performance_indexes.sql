-- Performance optimization indexes
-- Additional strategic indexes based on query analysis

-- Composite indexes for agents
CREATE INDEX IF NOT EXISTS idx_agents_type_status ON agents(agent_type, status);
CREATE INDEX IF NOT EXISTS idx_agents_status_last_seen ON agents(status, last_seen);
CREATE INDEX IF NOT EXISTS idx_agents_name_type ON agents(name, agent_type);

-- Composite indexes for issues
CREATE INDEX IF NOT EXISTS idx_issues_status_priority ON issues(status, priority);
CREATE INDEX IF NOT EXISTS idx_issues_status_created ON issues(status, created_at);
CREATE INDEX IF NOT EXISTS idx_issues_assigned_status ON issues(assigned_agent_id, status) WHERE assigned_agent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_issues_priority_created ON issues(priority, created_at);
CREATE INDEX IF NOT EXISTS idx_issues_updated_status ON issues(updated_at, status);

-- Composite indexes for messages
CREATE INDEX IF NOT EXISTS idx_messages_sender_created ON messages(sender_id, created_at);
CREATE INDEX IF NOT EXISTS idx_messages_recipient_created ON messages(recipient_id, created_at) WHERE recipient_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_messages_type_created ON messages(message_type, created_at);
CREATE INDEX IF NOT EXISTS idx_messages_sender_type ON messages(sender_id, message_type);
CREATE INDEX IF NOT EXISTS idx_messages_delivered_created ON messages(delivered_at, created_at) WHERE delivered_at IS NOT NULL;

-- Composite indexes for knowledge
CREATE INDEX IF NOT EXISTS idx_knowledge_type_created ON knowledge(knowledge_type, created_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_created_by_type ON knowledge(created_by, knowledge_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_access_type ON knowledge(access_level, knowledge_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_version_updated ON knowledge(version, updated_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_created_by_updated ON knowledge(created_by, updated_at);

-- Composite indexes for system_prompts
CREATE INDEX IF NOT EXISTS idx_prompts_type_active ON system_prompts(prompt_type, is_active);
CREATE INDEX IF NOT EXISTS idx_prompts_active_updated ON system_prompts(is_active, updated_at);
CREATE INDEX IF NOT EXISTS idx_prompts_created_by_type ON system_prompts(created_by, prompt_type);
CREATE INDEX IF NOT EXISTS idx_prompts_version_active ON system_prompts(version, is_active);

-- Composite indexes for agent_templates
CREATE INDEX IF NOT EXISTS idx_templates_type_active ON agent_templates(agent_type, is_active);
CREATE INDEX IF NOT EXISTS idx_templates_active_updated ON agent_templates(is_active, updated_at);
CREATE INDEX IF NOT EXISTS idx_templates_created_by_type ON agent_templates(created_by, agent_type);
CREATE INDEX IF NOT EXISTS idx_templates_prompt_active ON agent_templates(system_prompt_id, is_active) WHERE system_prompt_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_templates_version_active ON agent_templates(version, is_active);

-- Full-text search indexes for knowledge content (if supported)
-- Note: SQLite FTS5 extension may not be available in all environments
-- CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(title, content, content=knowledge, content_rowid=rowid);
-- CREATE TRIGGER IF NOT EXISTS knowledge_fts_insert AFTER INSERT ON knowledge BEGIN
--   INSERT INTO knowledge_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
-- END;
-- CREATE TRIGGER IF NOT EXISTS knowledge_fts_delete AFTER DELETE ON knowledge BEGIN
--   INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content) VALUES('delete', old.rowid, old.title, old.content);
-- END;
-- CREATE TRIGGER IF NOT EXISTS knowledge_fts_update AFTER UPDATE ON knowledge BEGIN
--   INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content) VALUES('delete', old.rowid, old.title, old.content);
--   INSERT INTO knowledge_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
-- END;

-- Performance optimization pragmas (these will be set at connection time)
-- PRAGMA journal_mode = WAL;
-- PRAGMA synchronous = NORMAL;
-- PRAGMA cache_size = -64000;  -- 64MB cache
-- PRAGMA temp_store = memory;
-- PRAGMA mmap_size = 268435456;  -- 256MB mmap