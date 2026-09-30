-- name: record_execs
-- id: bdacb7e8ee01410eb1233754168c14ea

CREATE TABLE execs (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    parent_exec_id TEXT,
    via_agent INTEGER CHECK (via_agent IN (0, 1)),
    caller_session_id TEXT,
    caller_provider_generation INTEGER,
    command TEXT,
    repo TEXT,
    cwd TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'interrupted')),
    exit_code INTEGER,
    error TEXT,
    signal TEXT
);
CREATE INDEX execs_parent ON execs(parent_exec_id, started_at, id);
CREATE INDEX execs_trace ON execs(trace_id, started_at, id);
CREATE INDEX execs_recent ON execs(started_at DESC, id);

