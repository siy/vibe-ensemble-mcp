-- Add project_prefix field to projects table for human-friendly ticket IDs
-- Format: Uppercase letters from repository name (e.g., "todo-vue-rust" → "TVR")

ALTER TABLE projects ADD COLUMN project_prefix TEXT;

-- Generate and update prefix for existing projects
UPDATE projects
SET project_prefix = (
    SELECT UPPER(
        SUBSTR(repository_name, 1, 1) ||
        CASE
            WHEN INSTR(repository_name, '-') > 0 THEN
                SUBSTR(repository_name, INSTR(repository_name, '-') + 1, 1) ||
                CASE
                    WHEN INSTR(SUBSTR(repository_name, INSTR(repository_name, '-') + 1), '-') > 0 THEN
                        SUBSTR(SUBSTR(repository_name, INSTR(repository_name, '-') + 1),
                               INSTR(SUBSTR(repository_name, INSTR(repository_name, '-') + 1), '-') + 1, 1)
                    ELSE ''
                END
            ELSE ''
        END
    )
)
WHERE project_prefix IS NULL;

-- Make project_prefix NOT NULL after populating existing rows
-- Note: SQLite doesn't support ALTER COLUMN, so we'll handle this in application code
-- by ensuring new projects always have project_prefix generated
