-- Keep terminal Project Sessions as history while allowing one current
-- successor for the same Linear Project. SQLite bakes a column UNIQUE
-- constraint into the table, so removing it requires the documented table
-- rebuild rather than dropping an index.

CREATE TABLE project_sessions_next (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    project_prompt_context TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    pm_snapshot_synced_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    observation_cursor INTEGER NOT NULL,
    last_state_fingerprint TEXT,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    current_directive_version INTEGER NOT NULL,
    incorporated_directive_version INTEGER NOT NULL,
    lf_bin TEXT,
    db_path TEXT,
    lf_home TEXT,
    abandon_requested_at INTEGER,
    abandon_reason TEXT
);

INSERT INTO project_sessions_next (
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status, status_reason, status_at,
    iteration, observation_cursor, last_state_fingerprint, agent, provider,
    provider_session_id, process_generation, process_pid, process_tmux_name,
    process_started_at, created_at, updated_at, current_directive_version,
    incorporated_directive_version, lf_bin, db_path, lf_home,
    abandon_requested_at, abandon_reason
)
SELECT
    id, project_id, project_slug, project_name, project_prompt_context,
    wave_id, pm_snapshot_synced_at, status, status_reason, status_at,
    iteration, observation_cursor, last_state_fingerprint, agent, provider,
    provider_session_id, process_generation, process_pid, process_tmux_name,
    process_started_at, created_at, updated_at, current_directive_version,
    incorporated_directive_version, lf_bin, db_path, lf_home,
    abandon_requested_at, abandon_reason
FROM project_sessions;

DROP TABLE project_sessions;
ALTER TABLE project_sessions_next RENAME TO project_sessions;

CREATE INDEX idx_project_sessions_wave_status
    ON project_sessions(wave_id, status, updated_at DESC);
CREATE UNIQUE INDEX idx_project_sessions_one_current
    ON project_sessions(project_id)
    WHERE status NOT IN ('completed', 'abandoned');
