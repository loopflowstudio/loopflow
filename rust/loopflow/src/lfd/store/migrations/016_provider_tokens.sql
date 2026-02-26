CREATE TABLE IF NOT EXISTS provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER,
    login TEXT,
    updated_at INTEGER NOT NULL
);
