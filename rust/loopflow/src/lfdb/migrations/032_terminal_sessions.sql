CREATE TABLE IF NOT EXISTS terminal_sessions (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    run_id TEXT REFERENCES runs(id),
    step TEXT NOT NULL,
    agent TEXT NOT NULL,
    cwd TEXT NOT NULL,
    argv TEXT NOT NULL,
    env TEXT NOT NULL,
    source TEXT NOT NULL,
    status INTEGER NOT NULL,
    completion_token TEXT,
    created_at BIGINT NOT NULL,
    attached_at BIGINT,
    started_at BIGINT,
    completed_at BIGINT
);

CREATE INDEX IF NOT EXISTS idx_terminal_sessions_wave_id
    ON terminal_sessions(wave_id);

CREATE INDEX IF NOT EXISTS idx_terminal_sessions_run_id
    ON terminal_sessions(run_id);

CREATE INDEX IF NOT EXISTS idx_terminal_sessions_status
    ON terminal_sessions(status);
