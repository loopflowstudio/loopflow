-- name: record_session_events
-- id: b3d8bbde097141a596c4562954131457
-- depends_on: record_conversation_connection

-- Native receipts belong to the conversation, independently of driver exit.
-- A sequence identifies evidence, not another resumable execution object.
CREATE TABLE session_events (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    provider_thread TEXT NOT NULL,
    provider_turn TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('started', 'usage', 'completed')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    exec_id TEXT REFERENCES execs(id),
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    UNIQUE(session_id, provider_thread, provider_turn, kind, receipt_key)
);
CREATE INDEX session_events_history ON session_events(session_id, seq);
CREATE INDEX session_events_turn ON session_events(session_id, provider_thread, provider_turn, seq);
