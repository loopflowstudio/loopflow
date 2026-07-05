CREATE TABLE IF NOT EXISTS run_token_usage (
    run_id TEXT PRIMARY KEY,
    wave TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cache_read_tokens BIGINT NOT NULL DEFAULT 0,
    recorded_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_run_token_usage_wave_provider
ON run_token_usage(wave, provider);
