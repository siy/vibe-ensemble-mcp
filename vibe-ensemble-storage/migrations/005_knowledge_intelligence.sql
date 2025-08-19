-- Migration for advanced knowledge management and intelligence features
-- This migration adds tables for knowledge extraction, pattern recognition,
-- contribution workflows, suggestions, and capability enhancements

-- Table for extracted knowledge awaiting review
CREATE TABLE IF NOT EXISTS extracted_knowledge (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,           -- JSON: ExtractionSource
    source_id TEXT NOT NULL,             -- UUID of source (issue, message, etc.)
    extracted_title TEXT NOT NULL,
    extracted_content TEXT NOT NULL,
    confidence_score REAL NOT NULL DEFAULT 0.0,
    extraction_method TEXT NOT NULL,     -- JSON: ExtractionMethod
    suggested_type TEXT NOT NULL,        -- JSON: KnowledgeType
    suggested_tags TEXT NOT NULL,        -- JSON: Vec<String>
    extracted_by TEXT NOT NULL,          -- Agent ID who extracted
    extracted_at TEXT NOT NULL,          -- ISO 8601 timestamp
    review_status TEXT NOT NULL,         -- JSON: ReviewStatus
    quality_metrics TEXT NOT NULL,       -- JSON: QualityMetrics
    
    FOREIGN KEY (extracted_by) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for pending review queries
CREATE INDEX IF NOT EXISTS idx_extracted_knowledge_review_status 
ON extracted_knowledge(review_status);

-- Index for source queries
CREATE INDEX IF NOT EXISTS idx_extracted_knowledge_source 
ON extracted_knowledge(source_id);

-- Table for recognized patterns in agent interactions
CREATE TABLE IF NOT EXISTS recognized_patterns (
    id TEXT PRIMARY KEY,
    pattern_type TEXT NOT NULL,          -- JSON: PatternType
    pattern_name TEXT NOT NULL,
    pattern_description TEXT NOT NULL,
    frequency_count INTEGER NOT NULL DEFAULT 1,
    success_rate REAL NOT NULL DEFAULT 0.0,
    context_tags TEXT NOT NULL,          -- JSON: Vec<String>
    related_issues TEXT NOT NULL,        -- JSON: Vec<Uuid>
    related_agents TEXT NOT NULL,        -- JSON: Vec<Uuid>
    first_observed TEXT NOT NULL,        -- ISO 8601 timestamp
    last_observed TEXT NOT NULL,         -- ISO 8601 timestamp
    confidence_level REAL NOT NULL DEFAULT 0.0
);

-- Index for pattern type queries
CREATE INDEX IF NOT EXISTS idx_patterns_type 
ON recognized_patterns(pattern_type);

-- Index for mature pattern queries
CREATE INDEX IF NOT EXISTS idx_patterns_mature 
ON recognized_patterns(confidence_level, frequency_count);

-- Table for knowledge contribution workflow
CREATE TABLE IF NOT EXISTS knowledge_contributions (
    id TEXT PRIMARY KEY,
    contributor_id TEXT NOT NULL,        -- Agent who contributed
    knowledge_id TEXT,                   -- Existing knowledge being modified (nullable)
    extracted_knowledge_id TEXT,         -- Extracted knowledge being contributed (nullable)
    contribution_type TEXT NOT NULL,     -- JSON: ContributionType
    proposed_changes TEXT NOT NULL,      -- JSON: proposed changes/content
    workflow_status TEXT NOT NULL,       -- JSON: WorkflowStatus
    submitted_at TEXT NOT NULL,          -- ISO 8601 timestamp
    reviewed_at TEXT,                    -- ISO 8601 timestamp (nullable)
    approved_at TEXT,                    -- ISO 8601 timestamp (nullable)
    
    FOREIGN KEY (contributor_id) REFERENCES agents(id) ON DELETE CASCADE,
    FOREIGN KEY (knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (extracted_knowledge_id) REFERENCES extracted_knowledge(id) ON DELETE CASCADE
);

-- Index for workflow status queries
CREATE INDEX IF NOT EXISTS idx_contributions_status 
ON knowledge_contributions(workflow_status);

-- Index for contributor queries
CREATE INDEX IF NOT EXISTS idx_contributions_contributor 
ON knowledge_contributions(contributor_id);

-- Table for review comments on contributions
CREATE TABLE IF NOT EXISTS contribution_review_comments (
    id TEXT PRIMARY KEY,
    contribution_id TEXT NOT NULL,
    reviewer_id TEXT NOT NULL,           -- Agent who left the comment
    comment_text TEXT NOT NULL,
    comment_type TEXT NOT NULL,          -- JSON: CommentType
    created_at TEXT NOT NULL,            -- ISO 8601 timestamp
    
    FOREIGN KEY (contribution_id) REFERENCES knowledge_contributions(id) ON DELETE CASCADE,
    FOREIGN KEY (reviewer_id) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for contribution comments
CREATE INDEX IF NOT EXISTS idx_review_comments_contribution 
ON contribution_review_comments(contribution_id);

-- Table for knowledge suggestions
CREATE TABLE IF NOT EXISTS knowledge_suggestions (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,           -- JSON: SuggestionTarget
    target_id TEXT NOT NULL,             -- UUID of target (issue, agent, etc.)
    suggested_knowledge TEXT NOT NULL,   -- JSON: Vec<KnowledgeReference>
    suggestion_reason TEXT NOT NULL,
    confidence_score REAL NOT NULL,
    generated_by TEXT NOT NULL,          -- Agent/system that generated suggestion
    generated_at TEXT NOT NULL,          -- ISO 8601 timestamp
    accepted INTEGER,                    -- 1 for true, 0 for false, NULL for pending
    feedback TEXT,                       -- Optional feedback on suggestion
    
    FOREIGN KEY (generated_by) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for target suggestions
CREATE INDEX IF NOT EXISTS idx_suggestions_target 
ON knowledge_suggestions(target_type, target_id);

-- Index for pending suggestions
CREATE INDEX IF NOT EXISTS idx_suggestions_pending 
ON knowledge_suggestions(accepted) WHERE accepted IS NULL;

-- Table for agent capability enhancements
CREATE TABLE IF NOT EXISTS capability_enhancements (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    enhancement_type TEXT NOT NULL,      -- JSON: EnhancementType
    knowledge_sources TEXT NOT NULL,     -- JSON: Vec<Uuid> - knowledge that enabled enhancement
    capability_gained TEXT NOT NULL,     -- Description of new capability
    proficiency_level REAL NOT NULL DEFAULT 0.0,
    verification_method TEXT NOT NULL,   -- JSON: VerificationMethod
    enhanced_at TEXT NOT NULL,           -- ISO 8601 timestamp
    validated_at TEXT,                   -- ISO 8601 timestamp (nullable)
    
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for agent enhancement queries
CREATE INDEX IF NOT EXISTS idx_enhancements_agent 
ON capability_enhancements(agent_id);

-- Index for validated enhancements
CREATE INDEX IF NOT EXISTS idx_enhancements_validated 
ON capability_enhancements(validated_at) WHERE validated_at IS NOT NULL;

-- Table for organizational learning metrics
CREATE TABLE IF NOT EXISTS organizational_learning (
    id TEXT PRIMARY KEY,
    learning_period TEXT NOT NULL,       -- ISO 8601 timestamp
    knowledge_growth_rate REAL NOT NULL,
    pattern_discovery_rate REAL NOT NULL,
    knowledge_reuse_metrics TEXT NOT NULL, -- JSON: ReuseMetrics
    agent_improvement_metrics TEXT NOT NULL, -- JSON: Vec<AgentImprovement>
    organizational_insights TEXT NOT NULL,   -- JSON: Vec<String>
    recommended_focus_areas TEXT NOT NULL,   -- JSON: Vec<String>
    created_at TEXT NOT NULL             -- ISO 8601 timestamp
);

-- Index for learning period queries
CREATE INDEX IF NOT EXISTS idx_org_learning_period 
ON organizational_learning(learning_period);

-- Table for knowledge usage tracking (enhanced from existing)
CREATE TABLE IF NOT EXISTS knowledge_usage_events (
    id TEXT PRIMARY KEY,
    knowledge_id TEXT NOT NULL,
    used_by TEXT NOT NULL,               -- Agent ID
    usage_type TEXT NOT NULL,            -- JSON: enhanced usage types
    context_data TEXT,                   -- JSON: context information
    effectiveness_score REAL,            -- How effective was the knowledge
    feedback TEXT,                       -- Optional usage feedback
    created_at TEXT NOT NULL,            -- ISO 8601 timestamp
    
    FOREIGN KEY (knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (used_by) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for knowledge usage queries
CREATE INDEX IF NOT EXISTS idx_usage_events_knowledge 
ON knowledge_usage_events(knowledge_id);

-- Index for agent usage queries
CREATE INDEX IF NOT EXISTS idx_usage_events_agent 
ON knowledge_usage_events(used_by);

-- Table for enhanced message context analysis
CREATE TABLE IF NOT EXISTS message_context_analysis (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    primary_topics TEXT NOT NULL,        -- JSON: Vec<String>
    sentiment_score REAL NOT NULL,
    complexity_level TEXT NOT NULL,      -- JSON: ComplexityLevel
    urgency_indicators TEXT NOT NULL,    -- JSON: Vec<String>
    knowledge_gaps TEXT NOT NULL,        -- JSON: Vec<String>
    knowledge_references TEXT NOT NULL,  -- JSON: Vec<KnowledgeReference>
    suggested_actions TEXT NOT NULL,     -- JSON: Vec<String>
    analyzed_at TEXT NOT NULL,           -- ISO 8601 timestamp
    
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

-- Index for message analysis queries
CREATE INDEX IF NOT EXISTS idx_message_analysis_message 
ON message_context_analysis(message_id);

-- Table for pattern-issue relationships (many-to-many)
CREATE TABLE IF NOT EXISTS pattern_issue_relationships (
    pattern_id TEXT NOT NULL,
    issue_id TEXT NOT NULL,
    relationship_strength REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL,            -- ISO 8601 timestamp
    
    PRIMARY KEY (pattern_id, issue_id),
    FOREIGN KEY (pattern_id) REFERENCES recognized_patterns(id) ON DELETE CASCADE,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

-- Table for knowledge graph relationships (enhanced from existing)
CREATE TABLE IF NOT EXISTS knowledge_graph_edges (
    id TEXT PRIMARY KEY,
    source_knowledge_id TEXT NOT NULL,
    target_knowledge_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,     -- JSON: enhanced relationship types
    strength REAL NOT NULL DEFAULT 1.0,
    confidence REAL NOT NULL DEFAULT 1.0,
    created_by TEXT NOT NULL,            -- Agent or system that created relationship
    created_at TEXT NOT NULL,            -- ISO 8601 timestamp
    validated_at TEXT,                   -- ISO 8601 timestamp (nullable)
    
    FOREIGN KEY (source_knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (target_knowledge_id) REFERENCES knowledge(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES agents(id) ON DELETE CASCADE
);

-- Index for knowledge graph traversal
CREATE INDEX IF NOT EXISTS idx_knowledge_graph_source 
ON knowledge_graph_edges(source_knowledge_id);

CREATE INDEX IF NOT EXISTS idx_knowledge_graph_target 
ON knowledge_graph_edges(target_knowledge_id);

-- Table for automated quality assessments
CREATE TABLE IF NOT EXISTS quality_assessments (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,           -- 'knowledge', 'extracted_knowledge', etc.
    target_id TEXT NOT NULL,             -- UUID of assessed item
    assessment_type TEXT NOT NULL,       -- 'automated', 'peer_review', 'expert_review'
    quality_metrics TEXT NOT NULL,       -- JSON: QualityMetrics
    assessor_id TEXT,                    -- Agent ID (nullable for automated)
    assessment_notes TEXT,               -- Optional notes
    assessed_at TEXT NOT NULL,           -- ISO 8601 timestamp
    
    FOREIGN KEY (assessor_id) REFERENCES agents(id) ON DELETE SET NULL
);

-- Index for quality assessment queries
CREATE INDEX IF NOT EXISTS idx_quality_assessments_target 
ON quality_assessments(target_type, target_id);

-- Table for knowledge recommendation engine training data
CREATE TABLE IF NOT EXISTS recommendation_feedback (
    id TEXT PRIMARY KEY,
    suggestion_id TEXT NOT NULL,
    user_action TEXT NOT NULL,           -- 'accepted', 'rejected', 'modified'
    context_features TEXT NOT NULL,      -- JSON: features that influenced recommendation
    outcome_effectiveness REAL,          -- How effective was the recommendation
    feedback_notes TEXT,                 -- Optional user feedback
    created_at TEXT NOT NULL,            -- ISO 8601 timestamp
    
    FOREIGN KEY (suggestion_id) REFERENCES knowledge_suggestions(id) ON DELETE CASCADE
);

-- Index for recommendation training queries
CREATE INDEX IF NOT EXISTS idx_recommendation_feedback_suggestion 
ON recommendation_feedback(suggestion_id);

-- Views for common queries

-- View for knowledge quality overview
CREATE VIEW IF NOT EXISTS knowledge_quality_overview AS
SELECT 
    k.id,
    k.title,
    k.knowledge_type,
    k.created_at,
    qa.quality_metrics,
    qa.assessed_at,
    COUNT(ue.id) as usage_count,
    AVG(ue.effectiveness_score) as avg_effectiveness
FROM knowledge k
LEFT JOIN quality_assessments qa ON qa.target_id = k.id AND qa.target_type = 'knowledge'
LEFT JOIN knowledge_usage_events ue ON ue.knowledge_id = k.id
GROUP BY k.id, qa.id;

-- View for agent learning progress
CREATE VIEW IF NOT EXISTS agent_learning_progress AS
SELECT 
    a.id as agent_id,
    a.name as agent_name,
    COUNT(ce.id) as total_enhancements,
    COUNT(CASE WHEN ce.validated_at IS NOT NULL THEN 1 END) as validated_enhancements,
    AVG(ce.proficiency_level) as avg_proficiency,
    COUNT(kc.id) as knowledge_contributions,
    COUNT(CASE WHEN kc.workflow_status = '"Approved"' THEN 1 END) as approved_contributions
FROM agents a
LEFT JOIN capability_enhancements ce ON ce.agent_id = a.id
LEFT JOIN knowledge_contributions kc ON kc.contributor_id = a.id
GROUP BY a.id;

-- View for pattern effectiveness
CREATE VIEW IF NOT EXISTS pattern_effectiveness AS
SELECT 
    rp.id,
    rp.pattern_name,
    rp.pattern_type,
    rp.frequency_count,
    rp.success_rate,
    rp.confidence_level,
    COUNT(pir.issue_id) as related_issues_count,
    COUNT(ks.id) as suggestion_count,
    COUNT(CASE WHEN ks.accepted = 1 THEN 1 END) as accepted_suggestions
FROM recognized_patterns rp
LEFT JOIN pattern_issue_relationships pir ON pir.pattern_id = rp.id
LEFT JOIN knowledge_suggestions ks ON ks.suggested_knowledge LIKE '%' || rp.id || '%'
GROUP BY rp.id;