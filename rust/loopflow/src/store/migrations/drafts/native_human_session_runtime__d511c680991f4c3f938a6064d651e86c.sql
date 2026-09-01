-- name: native_human_session_runtime
-- id: d511c680991f4c3f938a6064d651e86c
-- depends_on: stable_work_state

DROP TABLE ask_linear_comment_outbox;
DROP TABLE ask_exchanges;

CREATE TABLE work_flow_positions (
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    flow TEXT NOT NULL,
    step TEXT NOT NULL,
    node_id TEXT,
    human INTEGER NOT NULL CHECK (human IN (0, 1)),
    session_run_id TEXT,
    ready_summary TEXT,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (work_kind, work_id)
);
