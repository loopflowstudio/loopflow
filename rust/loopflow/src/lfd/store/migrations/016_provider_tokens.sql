-- TODO: Encrypt provider tokens at rest once key management is available.
CREATE TABLE IF NOT EXISTS provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at BIGINT,
    login TEXT,
    updated_at BIGINT NOT NULL
);
