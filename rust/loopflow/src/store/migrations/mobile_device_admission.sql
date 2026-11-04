-- Device grants are local Machine authority, never copied as Work records.
CREATE TABLE machine_pairings (
    ticket_hash BLOB PRIMARY KEY CHECK(length(ticket_hash) = 32),
    name TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE machine_devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE CHECK(length(token_hash) = 32),
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);

-- Shared CLI admission, not a cache owned by the mobile carrier. A reservation
-- precedes Task creation; start_attempted precedes the non-repeatable Graph run.
CREATE TABLE task_create_requests (
    id TEXT PRIMARY KEY,
    request_json TEXT NOT NULL,
    task_id TEXT NOT NULL UNIQUE,
    start_attempted INTEGER NOT NULL DEFAULT 0 CHECK(start_attempted IN (0, 1)),
    result_json TEXT
);
