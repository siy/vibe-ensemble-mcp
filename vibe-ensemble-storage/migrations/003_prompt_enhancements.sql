-- Prompt management enhancements for metrics, A/B testing, feedback, and caching

-- Prompt metrics table for performance tracking
CREATE TABLE IF NOT EXISTS prompt_metrics (
    id TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL,
    agent_id TEXT,
    usage_count INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 0.0,
    average_response_time_ms REAL NOT NULL DEFAULT 0.0,
    quality_score REAL,
    user_feedback_score REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    FOREIGN KEY (prompt_id) REFERENCES system_prompts(id),
    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

-- Prompt experiments table for A/B testing
CREATE TABLE IF NOT EXISTS prompt_experiments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    prompt_a_id TEXT NOT NULL,
    prompt_b_id TEXT NOT NULL,
    allocation_percentage REAL NOT NULL DEFAULT 50.0,
    status TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    minimum_sample_size INTEGER NOT NULL DEFAULT 100,
    statistical_significance REAL,
    FOREIGN KEY (prompt_a_id) REFERENCES system_prompts(id),
    FOREIGN KEY (prompt_b_id) REFERENCES system_prompts(id),
    FOREIGN KEY (created_by) REFERENCES agents(id)
);

-- Prompt feedback table for quality assessment
CREATE TABLE IF NOT EXISTS prompt_feedback (
    id TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL,
    agent_id TEXT,
    task_id TEXT,
    feedback_type TEXT NOT NULL,
    score REAL NOT NULL,
    comments TEXT,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (prompt_id) REFERENCES system_prompts(id),
    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

-- Prompt cache table for performance optimization
CREATE TABLE IF NOT EXISTS prompt_cache (
    key TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL,
    variables_hash TEXT NOT NULL,
    rendered_content TEXT NOT NULL,
    cached_at TEXT NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 1,
    last_accessed TEXT NOT NULL,
    expires_at TEXT,
    FOREIGN KEY (prompt_id) REFERENCES system_prompts(id)
);

-- Indexes for better performance
CREATE INDEX IF NOT EXISTS idx_metrics_prompt ON prompt_metrics(prompt_id);
CREATE INDEX IF NOT EXISTS idx_metrics_agent ON prompt_metrics(agent_id);
CREATE INDEX IF NOT EXISTS idx_metrics_period ON prompt_metrics(period_start, period_end);
CREATE INDEX IF NOT EXISTS idx_metrics_created ON prompt_metrics(created_at);

CREATE INDEX IF NOT EXISTS idx_experiments_status ON prompt_experiments(status);
CREATE INDEX IF NOT EXISTS idx_experiments_dates ON prompt_experiments(start_date, end_date);
CREATE INDEX IF NOT EXISTS idx_experiments_prompts ON prompt_experiments(prompt_a_id, prompt_b_id);
CREATE INDEX IF NOT EXISTS idx_experiments_created_by ON prompt_experiments(created_by);

CREATE INDEX IF NOT EXISTS idx_feedback_prompt ON prompt_feedback(prompt_id);
CREATE INDEX IF NOT EXISTS idx_feedback_agent ON prompt_feedback(agent_id);
CREATE INDEX IF NOT EXISTS idx_feedback_type ON prompt_feedback(feedback_type);
CREATE INDEX IF NOT EXISTS idx_feedback_score ON prompt_feedback(score);
CREATE INDEX IF NOT EXISTS idx_feedback_created ON prompt_feedback(created_at);

CREATE INDEX IF NOT EXISTS idx_cache_prompt ON prompt_cache(prompt_id);
CREATE INDEX IF NOT EXISTS idx_cache_hash ON prompt_cache(variables_hash);
CREATE INDEX IF NOT EXISTS idx_cache_accessed ON prompt_cache(last_accessed);
CREATE INDEX IF NOT EXISTS idx_cache_expires ON prompt_cache(expires_at);