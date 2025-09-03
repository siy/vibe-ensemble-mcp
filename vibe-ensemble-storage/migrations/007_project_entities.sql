-- Database Schema Enhancement for Projects (Issue #132)
--
-- This migration adds project entities to enable project-based agent organization
-- and coordination in the Vibe Ensemble system.

-- Create projects table
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    working_directory TEXT,
    git_repository TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    status TEXT NOT NULL DEFAULT 'Active'
);

-- Update agents table to add project association
ALTER TABLE agents ADD COLUMN project_id TEXT REFERENCES projects(id);

-- Add performance indexes for projects
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status);
CREATE INDEX IF NOT EXISTS idx_projects_created_at ON projects(created_at);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);

-- Add performance index for agent-project relationships
CREATE INDEX IF NOT EXISTS idx_agents_project ON agents(project_id);