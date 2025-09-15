-- Migration 003: Add project rules and patterns to projects table
-- These columns allow coordinators to define coding standards and conventions
-- that are automatically inherited by all workers in the project

-- Add project_rules column (nullable)
-- Contains high-level project rules and conventions (e.g., "Use TypeScript for frontend, Rust for backend")
ALTER TABLE projects 
ADD COLUMN project_rules TEXT;

-- Add project_patterns column (nullable)  
-- Contains specific coding patterns and templates (e.g., "Components in /src/components, API routes in /api")
ALTER TABLE projects 
ADD COLUMN project_patterns TEXT;

-- Note: These columns are nullable to maintain backward compatibility
-- Existing projects will have NULL values, which is handled gracefully by the application
-- New projects can optionally specify rules and patterns for their workers