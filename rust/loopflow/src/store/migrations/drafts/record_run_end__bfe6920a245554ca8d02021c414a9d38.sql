-- name: record_run_end
-- id: bfe6920a245554ca8d02021c414a9d38
-- depends_on: record_ask_sessions

-- A Run's state is its row: no end means it has not settled. Its events,
-- transcript and usage stay in its record.
ALTER TABLE runs ADD COLUMN outcome TEXT
    CHECK (outcome IN ('completed', 'failed', 'interrupted'));
ALTER TABLE runs ADD COLUMN ended_at INTEGER
    CHECK ((ended_at IS NULL) = (outcome IS NULL));
CREATE INDEX run_caller_history ON runs(caller_run_id, created_at, id);
