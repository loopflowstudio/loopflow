CREATE TABLE creation_receipts (
    request_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    task_id TEXT,
    session_id TEXT,
    artifact_key TEXT,
    milestone TEXT NOT NULL,
    result TEXT,
    launch TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
