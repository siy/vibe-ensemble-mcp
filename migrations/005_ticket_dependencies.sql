-- Ticket Dependencies and DAG Support Migration
-- Adds support for ticket dependencies, hierarchical relationships, and project rules/patterns

-- Create ticket dependencies table with strict constraints
CREATE TABLE IF NOT EXISTS ticket_dependencies (
    parent_ticket_id TEXT NOT NULL,
    child_ticket_id TEXT NOT NULL,
    dependency_type TEXT NOT NULL DEFAULT 'blocks' CHECK (dependency_type IN ('blocks', 'subtask')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (parent_ticket_id, child_ticket_id),
    FOREIGN KEY (parent_ticket_id) REFERENCES tickets(ticket_id) ON DELETE CASCADE,
    FOREIGN KEY (child_ticket_id) REFERENCES tickets(ticket_id) ON DELETE CASCADE,
    -- Prevent self-dependencies
    CHECK (parent_ticket_id != child_ticket_id)
);

-- Enhance tickets table with dependency and hierarchy support
ALTER TABLE tickets ADD COLUMN parent_ticket_id TEXT REFERENCES tickets(ticket_id);
ALTER TABLE tickets ADD COLUMN dependency_status TEXT NOT NULL DEFAULT 'ready' CHECK (dependency_status IN ('ready', 'blocked', 'waiting'));
ALTER TABLE tickets ADD COLUMN created_by_worker_id TEXT;
ALTER TABLE tickets ADD COLUMN ticket_type TEXT NOT NULL DEFAULT 'task' CHECK (ticket_type IN ('epic', 'story', 'task', 'subtask'));
ALTER TABLE tickets ADD COLUMN rules_version INTEGER DEFAULT 1;
ALTER TABLE tickets ADD COLUMN patterns_version INTEGER DEFAULT 1;
ALTER TABLE tickets ADD COLUMN inherited_from_parent BOOLEAN NOT NULL DEFAULT 0;

-- Rename existing project_rules/project_patterns to remove redundant prefix
-- and add versioning support
ALTER TABLE projects RENAME COLUMN project_rules TO rules;
ALTER TABLE projects RENAME COLUMN project_patterns TO patterns;
ALTER TABLE projects ADD COLUMN rules_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE projects ADD COLUMN patterns_version INTEGER NOT NULL DEFAULT 1;

-- Performance indexes for dependency operations
CREATE INDEX IF NOT EXISTS idx_ticket_dependencies_parent ON ticket_dependencies(parent_ticket_id);
CREATE INDEX IF NOT EXISTS idx_ticket_dependencies_child ON ticket_dependencies(child_ticket_id);
CREATE INDEX IF NOT EXISTS idx_tickets_parent ON tickets(parent_ticket_id);
CREATE INDEX IF NOT EXISTS idx_tickets_dependency_status ON tickets(dependency_status);
CREATE INDEX IF NOT EXISTS idx_tickets_type ON tickets(ticket_type);
CREATE INDEX IF NOT EXISTS idx_tickets_created_by_worker ON tickets(created_by_worker_id);

-- Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_tickets_project_dependency_status ON tickets(project_id, dependency_status);
CREATE INDEX IF NOT EXISTS idx_tickets_parent_type ON tickets(parent_ticket_id, ticket_type);

-- Insert this migration version
INSERT OR IGNORE INTO schema_migrations (version) VALUES (5);