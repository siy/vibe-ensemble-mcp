-- Knowledge management enhancements for Issue #9
-- Adds relationship mapping, versioning history, and full-text search capabilities

-- Knowledge relationships table
CREATE TABLE IF NOT EXISTS knowledge_relations (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    FOREIGN KEY (source_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES agents(id),
    UNIQUE(source_id, target_id, relation_type)
);

-- Knowledge version history table
CREATE TABLE IF NOT EXISTS knowledge_versions (
    id TEXT PRIMARY KEY,
    knowledge_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    change_summary TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES agents(id),
    UNIQUE(knowledge_id, version)
);

-- Knowledge usage tracking
CREATE TABLE IF NOT EXISTS knowledge_usage (
    id TEXT PRIMARY KEY,
    knowledge_id TEXT NOT NULL,
    used_by TEXT NOT NULL,
    usage_type TEXT NOT NULL, -- viewed, referenced, applied, etc.
    context_data TEXT, -- JSON with additional context
    created_at TEXT NOT NULL,
    FOREIGN KEY (knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (used_by) REFERENCES agents(id)
);

-- Knowledge collections/categories
CREATE TABLE IF NOT EXISTS knowledge_collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    collection_type TEXT NOT NULL, -- topic, project, team, etc.
    metadata TEXT NOT NULL, -- JSON with additional metadata
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (created_by) REFERENCES agents(id)
);

-- Many-to-many relationship between knowledge and collections
CREATE TABLE IF NOT EXISTS knowledge_collection_members (
    knowledge_id TEXT NOT NULL,
    collection_id TEXT NOT NULL,
    added_by TEXT NOT NULL,
    added_at TEXT NOT NULL,
    PRIMARY KEY (knowledge_id, collection_id),
    FOREIGN KEY (knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE,
    FOREIGN KEY (added_by) REFERENCES agents(id)
);

-- Virtual table for full-text search on knowledge content
CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
    id UNINDEXED,
    title,
    content,
    tags,
    content='knowledge',
    content_rowid='rowid'
);

-- Trigger to keep FTS table in sync with knowledge table
CREATE TRIGGER IF NOT EXISTS knowledge_fts_insert AFTER INSERT ON knowledge BEGIN
  INSERT INTO knowledge_fts(id, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS knowledge_fts_delete AFTER DELETE ON knowledge BEGIN
  DELETE FROM knowledge_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS knowledge_fts_update AFTER UPDATE ON knowledge BEGIN
  DELETE FROM knowledge_fts WHERE id = old.id;
  INSERT INTO knowledge_fts(id, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
END;

-- Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_source ON knowledge_relations(source_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_target ON knowledge_relations(target_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_type ON knowledge_relations(relation_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_relations_created ON knowledge_relations(created_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_versions_knowledge ON knowledge_versions(knowledge_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_versions_version ON knowledge_versions(knowledge_id, version);
CREATE INDEX IF NOT EXISTS idx_knowledge_versions_created ON knowledge_versions(created_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_usage_knowledge ON knowledge_usage(knowledge_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_usage_user ON knowledge_usage(used_by);
CREATE INDEX IF NOT EXISTS idx_knowledge_usage_type ON knowledge_usage(usage_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_usage_created ON knowledge_usage(created_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_collections_type ON knowledge_collections(collection_type);
CREATE INDEX IF NOT EXISTS idx_knowledge_collections_created ON knowledge_collections(created_at);

CREATE INDEX IF NOT EXISTS idx_collection_members_collection ON knowledge_collection_members(collection_id);
CREATE INDEX IF NOT EXISTS idx_collection_members_added ON knowledge_collection_members(added_at);