-- name: record_execs
-- id: 8e1f10371bfa03133ea590b179afc4cf
-- depends_on: record_ask_sessions

CREATE TABLE execs (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    parent_exec_id TEXT,
    via_agent INTEGER CHECK (via_agent IN (0, 1)),
    caller_session_id TEXT,
    caller_provider_generation INTEGER,
    command TEXT,
    repo TEXT,
    cwd TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'interrupted')),
    exit_code INTEGER,
    signal TEXT
);
CREATE INDEX execs_parent ON execs(parent_exec_id, started_at, id);
CREATE INDEX execs_trace ON execs(trace_id, started_at, id);
CREATE INDEX execs_recent ON execs(started_at DESC, id);

ALTER TABLE sessions ADD COLUMN driver_exec_id TEXT REFERENCES execs(id);
ALTER TABLE sessions ADD COLUMN driver_generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN provider_generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN provider_exec_id TEXT REFERENCES execs(id);

-- The journal identifies actual lf processes. Old Run rows do not, and must
-- never manufacture Execs. Missing command completion remains missing.
INSERT INTO execs(id, trace_id, parent_exec_id, command, repo, cwd, started_at)
SELECT process_id, run_id, parent_process_id, command, repo, worktree, ts
FROM run_events AS event
WHERE seq = (SELECT MIN(seq) FROM run_events WHERE process_id = event.process_id)
GROUP BY process_id;

UPDATE execs SET
    completed_at = (SELECT ts FROM run_events WHERE process_id=execs.id
        AND node='run' AND event IN ('completed', 'errored') ORDER BY seq LIMIT 1),
    outcome = (SELECT CASE event WHEN 'completed' THEN 'succeeded'
        WHEN 'errored' THEN 'failed' END
        FROM run_events WHERE process_id=execs.id AND node='run'
        AND event IN ('completed', 'errored') ORDER BY seq LIMIT 1);
