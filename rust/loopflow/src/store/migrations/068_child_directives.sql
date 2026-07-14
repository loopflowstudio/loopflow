ALTER TABLE project_sessions
ADD COLUMN current_directive_version INTEGER NOT NULL DEFAULT 0;

ALTER TABLE project_sessions
ADD COLUMN incorporated_directive_version INTEGER NOT NULL DEFAULT 0;

ALTER TABLE task_sessions
ADD COLUMN current_directive_version INTEGER NOT NULL DEFAULT 0;

ALTER TABLE task_sessions
ADD COLUMN incorporated_directive_version INTEGER NOT NULL DEFAULT 0;

CREATE TABLE child_directives (
    id TEXT PRIMARY KEY,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    source_json TEXT NOT NULL,
    command_id TEXT,
    issued_at INTEGER NOT NULL,
    applied_at INTEGER,
    incorporated_at INTEGER,
    incorporated_summary TEXT,
    UNIQUE(target_kind, target_id, version)
);

CREATE INDEX idx_child_directives_target
ON child_directives(target_kind, target_id, version);

CREATE UNIQUE INDEX idx_child_directives_command
ON child_directives(command_id)
WHERE command_id IS NOT NULL;
