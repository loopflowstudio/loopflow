CREATE TABLE IF NOT EXISTS summaries (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    token_budget INTEGER NOT NULL,
    model TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_summaries_wave_id ON summaries(wave_id);
