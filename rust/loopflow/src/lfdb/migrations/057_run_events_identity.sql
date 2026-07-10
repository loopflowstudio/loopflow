-- The run ledger gets span identity. Pre-contract rows are dropped rather than
-- carried: their cost is undercounted, their repo is a basename, their node
-- vocabulary is split, and none of them can be attributed to a process. The
-- per-repo file journals remain the durable record.
--
-- Still no primary key: several writers share a run_id (a child `lf` inherits
-- LF_RUN_ID) -- that is the trace. process_id is the span, one per process.
DROP TABLE IF EXISTS run_events;

CREATE TABLE run_events (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    parent_process_id TEXT,
    seq BIGINT NOT NULL,
    ts BIGINT NOT NULL,
    repo TEXT,
    worktree TEXT,
    wave TEXT,
    node TEXT NOT NULL CHECK (node IN ('run', 'flow', 'skill')),
    event TEXT NOT NULL CHECK (event IN ('started', 'completed', 'errored', 'escalated')),
    command TEXT,
    flow TEXT,
    skill TEXT,
    step_index BIGINT,
    error TEXT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    cache_read_tokens BIGINT,
    cost_usd REAL,
    duration_secs REAL,
    provider TEXT,
    model TEXT,
    context TEXT
);

CREATE INDEX idx_run_events_ts ON run_events(ts);
CREATE INDEX idx_run_events_run ON run_events(run_id);
CREATE INDEX idx_run_events_process ON run_events(process_id);
