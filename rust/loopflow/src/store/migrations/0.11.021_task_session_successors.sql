-- Keep terminal Task Sessions as history while allowing one current successor
-- for the same issue id, identifier, and worktree. Mirrors 0.11.002
-- (project_session_successors): SQLite bakes a column UNIQUE constraint into
-- the table, so removing it requires the documented table rebuild rather than
-- dropping an index.
--
-- A Task is terminal when completed or abandoned (TaskSessionStatus::is_terminal).
-- `failed` is live and resumable, so a failed Task still holds the current slot —
-- you resume it, you do not succeed it. Only completed/abandoned free the key for
-- a successor.

CREATE TABLE task_sessions_next (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL,
    issue_identifier TEXT NOT NULL,
    issue_title TEXT NOT NULL,
    issue_description TEXT NOT NULL,
    project_id TEXT NOT NULL,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    project_prompt_context TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN (
        'created', 'starting', 'running', 'waiting', 'blocked', 'failed',
        'completed', 'abandoned'
    )),
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    worktree TEXT NOT NULL,
    workspace_slug TEXT NOT NULL,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    pm_snapshot_synced_at INTEGER NOT NULL,
    pm_writeback_json TEXT NOT NULL,
    project_session_id TEXT NOT NULL REFERENCES project_sessions(id) ON DELETE RESTRICT,
    current_directive_version INTEGER NOT NULL,
    incorporated_directive_version INTEGER NOT NULL,
    lf_bin TEXT,
    db_path TEXT,
    lf_home TEXT,
    abandon_requested_at INTEGER,
    abandon_reason TEXT,
    process_lease_token TEXT,
    process_group_id INTEGER,
    process_agent TEXT,
    process_provider TEXT,
    process_provider_session_id TEXT,
    process_lease_state TEXT
        CHECK (process_lease_state IN ('legacy', 'reserved', 'active', 'revoked', 'finished')),
    process_outcome_json TEXT,
    iterate_flow TEXT NOT NULL DEFAULT 'task',
    iterate_interaction_policy TEXT NOT NULL DEFAULT 'require'
        CHECK (iterate_interaction_policy IN ('require', 'defer')),
    phase_cursor INTEGER NOT NULL DEFAULT 0 CHECK (phase_cursor >= 0),
    phase_iteration INTEGER NOT NULL DEFAULT 0 CHECK (phase_iteration >= 0),
    kickoff_flow TEXT NOT NULL DEFAULT 'task-kickoff',
    kickoff_interaction_policy TEXT NOT NULL DEFAULT 'require'
        CHECK (kickoff_interaction_policy IN ('require', 'defer')),
    gate_flow TEXT NOT NULL DEFAULT 'task-gate',
    gate_interaction_policy TEXT NOT NULL DEFAULT 'require'
        CHECK (gate_interaction_policy IN ('require', 'defer')),
    lifecycle_phase TEXT NOT NULL DEFAULT 'iterate'
        CHECK (lifecycle_phase IN ('kickoff', 'iterate', 'gate')),
    phase_epoch INTEGER NOT NULL DEFAULT 1 CHECK (phase_epoch > 0),
    gate_cycle INTEGER NOT NULL DEFAULT 0 CHECK (gate_cycle >= 0),
    gate_proposal_json TEXT,
    process_provenance_json TEXT
);

INSERT INTO task_sessions_next
SELECT * FROM task_sessions;

DROP TABLE task_sessions;
ALTER TABLE task_sessions_next RENAME TO task_sessions;

CREATE INDEX idx_task_sessions_wave_status
    ON task_sessions(wave_id, status, updated_at DESC);

-- One current (non-terminal) successor per issue id, identifier, and worktree.
-- Terminal predecessors (completed/abandoned) are preserved as history and may
-- share the key with a live successor; resolution selects the live one.
CREATE UNIQUE INDEX idx_task_sessions_one_current_issue
    ON task_sessions(issue_id) WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_identifier
    ON task_sessions(issue_identifier) WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_worktree
    ON task_sessions(worktree) WHERE status NOT IN ('completed', 'abandoned');
