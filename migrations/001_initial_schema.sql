-- Initial schema migration for Vibe-Ensemble MCP
-- Creates all tables with clean, consistent structure

-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    repository_name TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    short_description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Worker types table
CREATE TABLE IF NOT EXISTS worker_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    worker_type TEXT NOT NULL,
    short_description TEXT,
    system_prompt TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE,
    UNIQUE(project_id, worker_type)
);

-- Tickets table with clean schema
CREATE TABLE IF NOT EXISTS tickets (
    ticket_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    execution_plan TEXT NOT NULL,
    current_stage TEXT NOT NULL DEFAULT 'planning',
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'closed', 'on_hold')),
    priority TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
    processing_worker_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at TEXT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
);

-- Comments table
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ticket_id TEXT NOT NULL,
    worker_type TEXT,
    worker_id TEXT,
    stage_number INTEGER,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (ticket_id) REFERENCES tickets(ticket_id) ON DELETE CASCADE
);

-- Workers table
CREATE TABLE IF NOT EXISTS workers (
    worker_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    worker_type TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('spawning', 'active', 'idle', 'finished', 'failed')),
    pid INTEGER,
    queue_name TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_activity TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
);

-- Events table
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('ticket_stage_completed', 'worker_stopped', 'task_assigned')),
    ticket_id TEXT,
    worker_id TEXT,
    stage TEXT,
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed BOOLEAN NOT NULL DEFAULT 0
);

-- Migration tracking table
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes for better performance on queue operations
CREATE INDEX IF NOT EXISTS idx_tickets_project_stage ON tickets(project_id, current_stage);
CREATE INDEX IF NOT EXISTS idx_tickets_state ON tickets(state);
CREATE INDEX IF NOT EXISTS idx_tickets_priority ON tickets(priority);
CREATE INDEX IF NOT EXISTS idx_workers_project_type ON workers(project_id, worker_type);
CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
CREATE INDEX IF NOT EXISTS idx_events_processed ON events(processed);

-- Insert this migration version
INSERT OR IGNORE INTO schema_migrations (version) VALUES (1);