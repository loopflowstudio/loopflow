CREATE TABLE secrets_provider_config (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    project TEXT,
    config TEXT,
    updated_at BIGINT NOT NULL,
    encrypted BOOLEAN NOT NULL DEFAULT 0
);
