CREATE TABLE IF NOT EXISTS secrets_provider_config (
    provider TEXT PRIMARY KEY,
    project TEXT,
    config TEXT,
    updated_at BIGINT NOT NULL DEFAULT 0
);
