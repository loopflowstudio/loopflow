-- Keep terminal Task Sessions as history while allowing one current successor
-- for the same Linear issue, worktree, and workspace. A durable Task is the
-- chain of task_sessions rows sharing one issue_id; recovery mints a successor
-- that adopts the predecessor's worktree and PR sequence. SQLite bakes a column
-- UNIQUE constraint into the table, so relaxing issue_id / issue_identifier /
-- worktree to "unique only among non-terminal Sessions" requires the documented
-- table rebuild rather than dropping an index (mirrors
-- 0.11.002_project_session_successors.sql). Foreign keys are off during
-- migration, so dropping the old table does not cascade task_prs / task_events;
-- ids are preserved, so the post-migration foreign_key_check passes.

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
    -- The abandoned Session this one was recovered from, giving the durable Task
    -- an exact predecessor/successor chain rather than one inferred from
    -- second-granular timestamps. NULL for a root (never-recovered) Session.
    predecessor_session_id TEXT
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
    process_provider_session_id, process_lease_state, process_outcome_json
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
    process_provider_session_id, process_lease_state, process_outcome_json
FROM task_sessions;

DROP TABLE task_sessions;
ALTER TABLE task_sessions_next RENAME TO task_sessions;

CREATE INDEX idx_task_sessions_wave_status
    ON task_sessions(wave_id, status, updated_at DESC);
-- History reads (predecessor/successor derivation) walk every Session for an issue.
CREATE INDEX idx_task_sessions_issue
    ON task_sessions(issue_id, created_at);
-- One current (non-terminal) attempt per issue / worktree / workspace identifier.
CREATE UNIQUE INDEX idx_task_sessions_one_current_issue
    ON task_sessions(issue_id)
    WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_identifier
    ON task_sessions(issue_identifier)
    WHERE status NOT IN ('completed', 'abandoned');
CREATE UNIQUE INDEX idx_task_sessions_one_current_worktree
    ON task_sessions(worktree)
    WHERE status NOT IN ('completed', 'abandoned');
