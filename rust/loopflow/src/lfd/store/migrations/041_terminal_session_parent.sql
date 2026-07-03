ALTER TABLE terminal_sessions ADD COLUMN parent_session_id TEXT REFERENCES terminal_sessions(id);

CREATE INDEX IF NOT EXISTS idx_terminal_sessions_parent_session_id
    ON terminal_sessions(parent_session_id);
