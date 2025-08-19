-- Security layer tables for authentication, authorization, and audit logging

-- Users table for authentication
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    email TEXT,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL, -- JSON serialized UserRole
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_login_at TEXT,
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT,
    created_by TEXT NOT NULL
);

-- Agent tokens table for API authentication
CREATE TABLE IF NOT EXISTS agent_tokens (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    permissions TEXT NOT NULL, -- JSON serialized permissions
    is_active BOOLEAN NOT NULL DEFAULT 1,
    expires_at TEXT,
    last_used_at TEXT,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    FOREIGN KEY (agent_id) REFERENCES agents(id),
    FOREIGN KEY (created_by) REFERENCES users(id)
);

-- Sessions table for web interface
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_data TEXT NOT NULL, -- JSON serialized session info
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_activity_at TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Audit events table for comprehensive logging
CREATE TABLE IF NOT EXISTS audit_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL, -- JSON serialized AuditEventType
    severity TEXT NOT NULL, -- JSON serialized AuditSeverity
    user_id TEXT,
    username TEXT,
    agent_id TEXT,
    resource_type TEXT,
    resource_id TEXT,
    action TEXT,
    description TEXT NOT NULL,
    metadata TEXT NOT NULL, -- JSON serialized HashMap<String, String>
    ip_address TEXT,
    user_agent TEXT,
    session_id TEXT,
    result TEXT NOT NULL, -- "success" or "failure"
    error_message TEXT,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

-- Encryption keys table for key management
CREATE TABLE IF NOT EXISTS encryption_keys (
    id TEXT PRIMARY KEY,
    key_data TEXT NOT NULL, -- Base64 encoded encrypted key
    created_at TEXT NOT NULL,
    expires_at TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    algorithm TEXT NOT NULL DEFAULT 'AES-256-GCM',
    created_by TEXT NOT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

-- Rate limit tracking table
CREATE TABLE IF NOT EXISTS rate_limit_tracking (
    id TEXT PRIMARY KEY,
    identifier TEXT NOT NULL, -- IP address or user ID
    identifier_type TEXT NOT NULL, -- 'ip' or 'user'
    endpoint_type TEXT NOT NULL, -- 'general', 'auth', 'api', etc.
    request_count INTEGER NOT NULL DEFAULT 0,
    window_start TEXT NOT NULL,
    last_request_at TEXT NOT NULL
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);

CREATE INDEX IF NOT EXISTS idx_agent_tokens_agent_id ON agent_tokens(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_tokens_active ON agent_tokens(is_active);
CREATE INDEX IF NOT EXISTS idx_agent_tokens_expires_at ON agent_tokens(expires_at);
CREATE INDEX IF NOT EXISTS idx_agent_tokens_created_by ON agent_tokens(created_by);

CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON sessions(last_activity_at);

CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_agent_id ON audit_events(agent_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_severity ON audit_events(severity);
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource ON audit_events(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_ip_address ON audit_events(ip_address);

CREATE INDEX IF NOT EXISTS idx_encryption_keys_active ON encryption_keys(is_active);
CREATE INDEX IF NOT EXISTS idx_encryption_keys_expires_at ON encryption_keys(expires_at);
CREATE INDEX IF NOT EXISTS idx_encryption_keys_created_by ON encryption_keys(created_by);

CREATE INDEX IF NOT EXISTS idx_rate_limit_identifier ON rate_limit_tracking(identifier, identifier_type, endpoint_type);
CREATE INDEX IF NOT EXISTS idx_rate_limit_window ON rate_limit_tracking(window_start);
CREATE INDEX IF NOT EXISTS idx_rate_limit_last_request ON rate_limit_tracking(last_request_at);

-- NOTE: Default admin credentials are NOT created automatically for security reasons.
-- To create an initial admin user, use the following SQL with a secure password:
--
-- EXAMPLE (use a secure random password):
-- INSERT INTO users (
--     id, username, email, password_hash, role, is_active, 
--     created_at, updated_at, created_by
-- ) VALUES (
--     'admin_user_001',
--     'admin',
--     'admin@vibeensemble.local',
--     '$2b$12$YOUR_SECURE_BCRYPT_HASH_HERE',
--     '"Admin"',
--     1,
--     datetime('now'),
--     datetime('now'),
--     'system'
-- );
--
-- To generate a secure password hash, use bcrypt with cost 12 or higher.
-- Never use default/weak passwords in production environments.

-- Create service account for system operations with disabled password login
-- This account can only be used for internal system operations, not for login
INSERT OR IGNORE INTO users (
    id, 
    username, 
    email, 
    password_hash, 
    role, 
    is_active, 
    created_at, 
    updated_at,
    created_by
) VALUES (
    'service_user_001',
    'system_service',
    'system@vibeensemble.local',
    'DISABLED_PASSWORD_LOGIN', -- Special marker indicating password login is disabled
    '"Coordinator"',
    1,
    datetime('now'),
    datetime('now'),
    'system'
);

-- Log the migration
INSERT OR IGNORE INTO audit_events (
    id,
    event_type,
    severity,
    description,
    metadata,
    result,
    timestamp
) VALUES (
    'migration_004_' || datetime('now'),
    '"DatabaseMigration"',
    '"Medium"',
    'Applied security layer migration 004',
    '{"migration": "004_security_layer.sql", "tables_created": "users,agent_tokens,sessions,audit_events,encryption_keys,rate_limit_tracking"}',
    'success',
    datetime('now')
);