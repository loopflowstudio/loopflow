CREATE TABLE IF NOT EXISTS connection_tokens (
    token_hash TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    claimed_at INTEGER,
    revoked_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_connection_tokens_state_expires
    ON connection_tokens(state, expires_at);
