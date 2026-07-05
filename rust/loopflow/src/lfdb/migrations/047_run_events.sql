-- Append-only run ledger. Deliberately no primary key: several writers may
-- share a run_id (a child `lf` inherits LF_RUN_ID from its parent), each with
-- its own seq counter. Ordering is (ts, seq).
CREATE TABLE IF NOT EXISTS run_events (
    run_id TEXT NOT NULL,
    seq BIGINT NOT NULL,
    ts BIGINT NOT NULL,
    repo TEXT,
    worktree TEXT,
    wave TEXT,
    node TEXT NOT NULL,
    event TEXT NOT NULL,
    command TEXT,
    flow TEXT,
    step TEXT,
    step_index BIGINT,
    error TEXT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    cache_read_tokens BIGINT,
    cost_usd REAL,
    duration_secs REAL
);

CREATE INDEX IF NOT EXISTS idx_run_events_ts ON run_events(ts);
CREATE INDEX IF NOT EXISTS idx_run_events_run ON run_events(run_id);
