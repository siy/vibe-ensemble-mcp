-- Migration 002: Expand event types to support enhanced coordinator events
-- Removes the CHECK constraint on event_type to allow new event types

-- Remove the CHECK constraint on event_type to allow new event types
-- Note: SQLite doesn't support dropping constraints directly, so we need to recreate the table

-- Create new events table without the restrictive CHECK constraint
CREATE TABLE events_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,  -- No CHECK constraint to allow flexible event types
    ticket_id TEXT,
    worker_id TEXT,
    stage TEXT,
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed BOOLEAN NOT NULL DEFAULT 0
);

-- Copy existing data
INSERT INTO events_new (id, event_type, ticket_id, worker_id, stage, reason, created_at, processed)
SELECT id, event_type, ticket_id, worker_id, stage, reason, created_at, processed
FROM events;

-- Drop old table and rename new one
DROP TABLE events;
ALTER TABLE events_new RENAME TO events;

-- Recreate index
CREATE INDEX IF NOT EXISTS idx_events_processed ON events(processed);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);

-- Insert this migration version
INSERT OR IGNORE INTO schema_migrations (version) VALUES (2);