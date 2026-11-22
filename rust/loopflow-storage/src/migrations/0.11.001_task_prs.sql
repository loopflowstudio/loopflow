-- Model every published or abandoned serial Task branch as a PR.

CREATE TABLE task_sessions_next (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL UNIQUE,
    issue_identifier TEXT NOT NULL UNIQUE,
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
    worktree TEXT NOT NULL UNIQUE,
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
    abandon_reason TEXT
);

INSERT INTO task_sessions_next (
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    status, status_reason, status_at, worktree, workspace_slug,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at, pm_writeback_json, project_session_id,
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason
)
SELECT
    id, issue_id, issue_identifier, issue_title, issue_description,
    project_id, project_slug, project_name, project_prompt_context, wave_id,
    CASE status
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE status
    END,
    status_reason, status_at, worktree,
    CASE
        WHEN instr(branch, '/') > 0 THEN substr(branch, instr(branch, '/') + 1)
        ELSE branch
    END,
    agent, provider, provider_session_id, process_generation, process_pid,
    process_tmux_name, process_started_at, created_at, updated_at,
    pm_snapshot_synced_at, pm_writeback_json, project_session_id,
    current_directive_version, incorporated_directive_version,
    lf_bin, db_path, lf_home, abandon_requested_at, abandon_reason
FROM task_sessions;

CREATE TABLE task_prs (
    id TEXT PRIMARY KEY,
    task_session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('review', 'complete_task')),
    next_slug TEXT,
    github_number INTEGER CHECK (github_number > 0),
    github_url TEXT,
    merge_commit TEXT,
    abandoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (task_session_id, sequence),
    CHECK ((publication_requested_at IS NULL) = (after_merge IS NULL)),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL)
);

INSERT INTO task_prs (
    id, task_session_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at
)
SELECT
    'pr_' || lower(hex(randomblob(16))), id, 1,
    CASE
        WHEN instr(branch, '/') > 0 THEN substr(branch, instr(branch, '/') + 1)
        ELSE branch
    END,
    branch, base_commit,
    CASE WHEN pr_number IS NOT NULL THEN updated_at ELSE NULL END,
    CASE
        WHEN status = 'merged' THEN 'complete_task'
        WHEN pr_number IS NOT NULL THEN 'review'
        ELSE NULL
    END,
    NULL, pr_number, pr_url,
    CASE status WHEN 'merged' THEN 'legacy-unknown' ELSE NULL END,
    CASE status WHEN 'abandoned' THEN updated_at ELSE NULL END,
    created_at, updated_at
FROM task_sessions;

-- Rewrite persisted event payloads to the PR-history wire shape.
UPDATE task_events
SET kind_json = json_set(
    kind_json,
    '$.from', CASE json_extract(kind_json, '$.from')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(kind_json, '$.from')
    END,
    '$.to', CASE json_extract(kind_json, '$.to')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(kind_json, '$.to')
    END
)
WHERE json_extract(kind_json, '$.kind') = 'status_changed';

UPDATE task_events
SET kind_json = json_set(
    kind_json,
    '$.kind', 'pr_opened',
    '$.pr_id', (
        SELECT id FROM task_prs
        WHERE task_session_id = task_events.session_id AND sequence = 1
    ),
    '$.sequence', 1
)
WHERE json_extract(kind_json, '$.kind') = 'pull_request_opened';

UPDATE task_events
SET kind_json = json_remove(kind_json, '$.pull_request')
WHERE json_extract(kind_json, '$.kind') = 'completed';

UPDATE observation_outbox
SET payload_json = json_set(
    payload_json,
    '$.event.from', CASE json_extract(payload_json, '$.event.from')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(payload_json, '$.event.from')
    END,
    '$.event.to', CASE json_extract(payload_json, '$.event.to')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(payload_json, '$.event.to')
    END
)
WHERE source_kind = 'task'
  AND json_extract(payload_json, '$.event.kind') = 'status_changed';

UPDATE observation_outbox
SET payload_json = json_set(
    payload_json,
    '$.event.kind', 'pr_opened',
    '$.event.pr_id', (
        SELECT id FROM task_prs
        WHERE task_session_id = observation_outbox.source_id AND sequence = 1
    ),
    '$.event.sequence', 1
)
WHERE source_kind = 'task'
  AND json_extract(payload_json, '$.event.kind') = 'pull_request_opened';

UPDATE observation_outbox
SET payload_json = json_remove(payload_json, '$.event.pull_request')
WHERE source_kind = 'task'
  AND json_extract(payload_json, '$.event.kind') = 'completed';

UPDATE project_events
SET kind_json = json_set(
    kind_json,
    '$.event.from', CASE json_extract(kind_json, '$.event.from')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(kind_json, '$.event.from')
    END,
    '$.event.to', CASE json_extract(kind_json, '$.event.to')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(kind_json, '$.event.to')
    END
)
WHERE json_extract(kind_json, '$.kind') = 'task_observed'
  AND json_extract(kind_json, '$.event.kind') = 'status_changed';

UPDATE project_events
SET kind_json = json_set(
    kind_json,
    '$.event.kind', 'pr_opened',
    '$.event.pr_id', (
        SELECT id FROM task_prs
        WHERE task_session_id = json_extract(project_events.kind_json, '$.task_session_id')
          AND sequence = 1
    ),
    '$.event.sequence', 1
)
WHERE json_extract(kind_json, '$.kind') = 'task_observed'
  AND json_extract(kind_json, '$.event.kind') = 'pull_request_opened';

UPDATE project_events
SET kind_json = json_remove(kind_json, '$.event.pull_request')
WHERE json_extract(kind_json, '$.kind') = 'task_observed'
  AND json_extract(kind_json, '$.event.kind') = 'completed';

UPDATE observation_outbox
SET payload_json = json_set(
    payload_json,
    '$.event.event.from', CASE json_extract(payload_json, '$.event.event.from')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(payload_json, '$.event.event.from')
    END,
    '$.event.event.to', CASE json_extract(payload_json, '$.event.event.to')
        WHEN 'submitted' THEN 'waiting'
        WHEN 'merged' THEN 'completed'
        ELSE json_extract(payload_json, '$.event.event.to')
    END
)
WHERE source_kind = 'project'
  AND json_extract(payload_json, '$.event.kind') = 'task_observed'
  AND json_extract(payload_json, '$.event.event.kind') = 'status_changed';

UPDATE observation_outbox
SET payload_json = json_set(
    payload_json,
    '$.event.event.kind', 'pr_opened',
    '$.event.event.pr_id', (
        SELECT id FROM task_prs
        WHERE task_session_id = json_extract(
            observation_outbox.payload_json,
            '$.event.task_session_id'
        ) AND sequence = 1
    ),
    '$.event.event.sequence', 1
)
WHERE source_kind = 'project'
  AND json_extract(payload_json, '$.event.kind') = 'task_observed'
  AND json_extract(payload_json, '$.event.event.kind') = 'pull_request_opened';

UPDATE observation_outbox
SET payload_json = json_remove(payload_json, '$.event.event.pull_request')
WHERE source_kind = 'project'
  AND json_extract(payload_json, '$.event.kind') = 'task_observed'
  AND json_extract(payload_json, '$.event.event.kind') = 'completed';

DROP TABLE task_sessions;
ALTER TABLE task_sessions_next RENAME TO task_sessions;

CREATE INDEX idx_task_sessions_wave_status
    ON task_sessions(wave_id, status, updated_at DESC);
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_session_id)
    WHERE merge_commit IS NULL AND abandoned_at IS NULL;
