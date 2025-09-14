-- Add project rules and patterns to projects table
-- These will be available to workers to understand project conventions

ALTER TABLE projects 
ADD COLUMN project_rules TEXT;

ALTER TABLE projects 
ADD COLUMN project_patterns TEXT;

-- Update the updated_at trigger to include new columns
-- Note: SQLite doesn't have built-in triggers, so we'll handle this in the application