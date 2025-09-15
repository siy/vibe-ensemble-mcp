-- Migration: Add resolution_summary field to events table
-- This field allows coordinators to document their investigation and actions
-- taken to resolve events that require attention.

ALTER TABLE events ADD COLUMN resolution_summary TEXT;

-- Insert this migration version
INSERT OR IGNORE INTO schema_migrations (version) VALUES (4);