CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    harness TEXT NOT NULL,
    status INTEGER NOT NULL,
    run_id TEXT,
    provider_session_id TEXT,
    config TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE TABLE IF NOT EXISTS conversation_events (
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(conversation_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_conversations_run_id ON conversations(run_id);
