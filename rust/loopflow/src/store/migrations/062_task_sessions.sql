CREATE TABLE task_sessions (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL UNIQUE,
    issue_identifier TEXT NOT NULL UNIQUE,
    issue_title TEXT NOT NULL,
    issue_description TEXT NOT NULL,
    project_id TEXT NOT NULL,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    wave_name TEXT NOT NULL,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    worktree TEXT NOT NULL UNIQUE,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    pr_number INTEGER,
    pr_url TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_task_sessions_wave_status
ON task_sessions(wave_id, status, updated_at DESC);

CREATE TABLE task_commands (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    source_json TEXT NOT NULL,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    claimed_by_generation INTEGER,
    acknowledged_at INTEGER
);

CREATE INDEX idx_task_commands_pending
ON task_commands(session_id, acknowledged_at, created_at, id);

CREATE TABLE task_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_task_events_session
ON task_events(session_id, id);
