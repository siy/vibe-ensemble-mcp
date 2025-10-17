-- Add JBCT (Java Backend Coding Technology) integration fields to projects table
-- Migration 007: Add jbct_enabled, jbct_version, jbct_url

ALTER TABLE projects ADD COLUMN jbct_enabled BOOLEAN DEFAULT FALSE NOT NULL;
ALTER TABLE projects ADD COLUMN jbct_version TEXT;
ALTER TABLE projects ADD COLUMN jbct_url TEXT;
