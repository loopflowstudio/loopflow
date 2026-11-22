-- A served command's first answer under its request ID, so a client that got
-- no reply can send it again and receive that answer instead of a second run.
CREATE TABLE command_answers (
    request_id TEXT PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    result TEXT,
    error TEXT,
    revisions TEXT,
    answered_at INTEGER NOT NULL,
    CHECK ((result IS NULL) <> (error IS NULL))
);
CREATE INDEX command_answers_answered_at ON command_answers(answered_at);
