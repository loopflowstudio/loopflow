-- Keep terminal Task Sessions as history while allowing one current successor
-- for the same Linear issue. A Task Session that completes or abandons leaves
-- its direction (Linear observation cursor, ingested-comment ledger) to be
-- carried or re-keyed onto a successor, instead of seeding the successor from
-- the launch snapshot and re-emitting or swallowing the predecessor's already-
-- applied edits. SQLite bakes a column UNIQUE constraint into the table, so
-- removing the issue_id / issue_identifier / worktree UNIQUEs requires the
-- documented table rebuild rather than dropping an index — the same shape as
-- 0.11.002 used for Project Sessions.

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
    phase_cursor INTEGER NOT NULL DEFAULT 0
        CHECK (phase_cursor >= 0),
    phase_iteration INTEGER NOT NULL DEFAULT 0
        CHECK (phase_iteration >= 0),
    kickoff_flow TEXT NOT NULL DEFAULT 'task-kickoff',
    kickoff_interaction_policy TEXT NOT NULL DEFAULT 'require'
        CHECK (kickoff_interaction_policy IN ('require', 'defer')),
    gate_flow TEXT NOT NULL DEFAULT 'task-gate',
    gate_interaction_policy TEXT NOT NULL DEFAULT 'require'
        CHECK (gate_interaction_policy IN ('require', 'defer')),
    lifecycle_phase TEXT NOT NULL DEFAULT 'iterate'
        CHECK (lifecycle_phase IN ('kickoff', 'iterate', 'gate')),
    phase_epoch INTEGER NOT NULL DEFAULT 1
        CHECK (phase_epoch > 0),
    gate_cycle INTEGER NOT NULL DEFAULT 0
        CHECK (gate_cycle >= 0),
    gate_proposal_json TEXT,
    process_provenance_json TEXT
);

INSERT INTO task_sessions_next (
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at, pm_writeback_json, project_session_id,
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_lease_token, process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
)
SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at, pm_writeback_json, project_session_id,
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason,
    process_lease_token, process_group_id, process_agent, process_provider,
    process_provider_session_id, process_lease_state, process_outcome_json,
    iterate_flow, iterate_interaction_policy, phase_cursor, phase_iteration,
    kickoff_flow, kickoff_interaction_policy, gate_flow, gate_interaction_policy,
    lifecycle_phase, phase_epoch, gate_cycle, gate_proposal_json,
    process_provenance_json
FROM task_sessions;

DROP TABLE task_sessions;
ALTER TABLE task_sessions_next RENAME TO task_sessions;

CREATE INDEX idx_task_sessions_wave_status
    ON task_sessions(wave_id, status, updated_at DESC);

-- At most one non-terminal Task Session per Linear issue identity and per
-- worktree. Terminal Sessions keep their rows for attributable history, so a
-- successor can coexist with its completed or abandoned predecessor.
CREATE UNIQUE INDEX idx_task_sessions_one_current_issue
    ON task_sessions(issue_id)
    WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_identifier
    ON task_sessions(issue_identifier)
    WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_worktree
    ON task_sessions(worktree)
    WHERE status NOT IN ('completed', 'abandoned');

-- Soft link from a successor to the terminal Session whose direction it
-- carries. Nullable: the first Session for an issue has no predecessor. Not a
-- hard FOREIGN KEY so ALTER TABLE can add it without rebuilding again; the
-- carry transaction is the only writer and points it at a real predecessor id.
ALTER TABLE task_sessions ADD COLUMN predecessor_session_id TEXT;
