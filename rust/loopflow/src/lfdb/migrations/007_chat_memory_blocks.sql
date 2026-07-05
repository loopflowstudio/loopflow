CREATE TABLE IF NOT EXISTS chat_memory_blocks (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    position INTEGER NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (wave_id, name)
);

CREATE INDEX IF NOT EXISTS idx_chat_memory_blocks_wave_pos
ON chat_memory_blocks(wave_id, position);
