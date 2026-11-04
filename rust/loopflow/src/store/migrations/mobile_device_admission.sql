-- Shared CLI admission, not a cache owned by the mobile carrier. A reservation
-- precedes Task creation; start_attempted precedes the non-repeatable Graph run.
CREATE TABLE task_create_requests (
    id TEXT PRIMARY KEY,
    request_json TEXT NOT NULL,
    task_id TEXT NOT NULL UNIQUE,
    start_attempted INTEGER NOT NULL DEFAULT 0 CHECK(start_attempted IN (0, 1)),
    result_json TEXT
);
