CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    status INTEGER NOT NULL,
    wave_run_id TEXT,
    provider_session_id TEXT,
    config TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE TABLE IF NOT EXISTS session_events (
    session_id TEXT NOT NULL REFERENCES sessions(id),
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(session_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_sessions_wave_run_id ON sessions(wave_run_id);
