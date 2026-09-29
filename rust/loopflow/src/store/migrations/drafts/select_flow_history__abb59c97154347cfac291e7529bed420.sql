-- name: select_flow_history
-- id: abb59c97154347cfac291e7529bed420
-- depends_on: retain_conversation_engine

ALTER TABLE flow_sessions ADD COLUMN selected_start INTEGER REFERENCES session_events(seq);
CREATE TABLE flow_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id TEXT NOT NULL REFERENCES flow_sessions(id),
    version INTEGER NOT NULL,
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('selected', 'consumed')),
    session_event INTEGER NOT NULL REFERENCES session_events(seq),
    observed_at INTEGER NOT NULL,
    UNIQUE(flow_id, kind, session_event)
);
CREATE INDEX flow_events_history ON flow_events(flow_id, seq);
