CREATE TABLE attention_items (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    run_id TEXT REFERENCES wave_runs(id),
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'surfaced',
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    context TEXT NOT NULL DEFAULT '{}',
    surfaced_at BIGINT NOT NULL,
    viewed_at BIGINT,
    resolved_at BIGINT
);

CREATE INDEX idx_attention_items_wave_id ON attention_items(wave_id);
CREATE INDEX idx_attention_items_status ON attention_items(status);
CREATE INDEX idx_attention_items_kind ON attention_items(kind);

DROP TABLE IF EXISTS wave_queue_blocks;
