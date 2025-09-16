-- Migration: Add resolution_summary field to events table
-- This field allows coordinators to document their investigation and actions
-- taken to resolve events that require attention.

ALTER TABLE events ADD COLUMN resolution_summary TEXT;

-- Add composite index for better query performance on processed events with ordering
CREATE INDEX IF NOT EXISTS idx_events_processed_created_at
  ON events(processed, created_at DESC);

-- Insert this migration version
INSERT OR IGNORE INTO schema_migrations (version) VALUES (4);