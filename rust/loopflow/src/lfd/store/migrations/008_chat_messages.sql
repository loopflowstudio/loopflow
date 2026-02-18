CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_wave_id
ON chat_messages(wave_id, created_at);
