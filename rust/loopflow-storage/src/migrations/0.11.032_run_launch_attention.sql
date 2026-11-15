-- Make product Run authority and opaque surfaces first-class on the existing
-- launch/turn lineage. Nullable control columns keep unattributed historical
-- trace captures honest instead of fabricating Run ownership for them.

ALTER TABLE agent_launches ADD COLUMN product_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN account_id TEXT;
ALTER TABLE agent_launches ADD COLUMN launch_state TEXT CHECK (
    launch_state IN ('starting', 'live', 'stopping', 'ended')
);
ALTER TABLE agent_launches ADD COLUMN containment_kind TEXT CHECK (
    containment_kind IN ('process_group', 'tmux')
);
ALTER TABLE agent_launches ADD COLUMN containment_id TEXT;
ALTER TABLE agent_launches ADD COLUMN resume_token TEXT;
ALTER TABLE agent_launches ADD COLUMN opaque_epoch_id TEXT REFERENCES epochs(id) ON DELETE RESTRICT;
ALTER TABLE agent_launches ADD COLUMN opaque_basis_rev INTEGER;
ALTER TABLE agent_launches ADD COLUMN attention_kind TEXT CHECK (
    attention_kind IN ('user', 'parent')
);
ALTER TABLE agent_launches ADD COLUMN attention_work_kind TEXT CHECK (
    attention_work_kind IN ('wave', 'project', 'task')
);
ALTER TABLE agent_launches ADD COLUMN attention_work_id TEXT;
ALTER TABLE agent_launches ADD COLUMN attention_at INTEGER;
ALTER TABLE agent_launches ADD COLUMN handback_state TEXT CHECK (
    handback_state IN ('succeeded', 'failed', 'interrupted', 'unknown')
);

UPDATE agent_launches SET resume_token = provider_session_id
WHERE provider_session_id IS NOT NULL;

-- The Session controller remains the temporary Task/Project executor. Give
-- every imported live body one product Launch at the process/tmux boundary it
-- already owns, so Run control never has to look back through Session state.
INSERT INTO agent_launches (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, project, task, provider, model, surface, capture_status,
    incomplete_reason, outcome, artifact_dir, conversation_path,
    provider_events_path, provider_session_id, provider_session_path,
    conversation_event_count, conversation_bytes, product_run_id, home_id,
    account_id, launch_state, containment_kind, containment_id, resume_token,
    opaque_epoch_id, opaque_basis_rev
)
SELECT
    'launch_' || lower(hex(randomblob(16))),
    runs.id,
    COALESCE(
        project_sessions.process_tmux_name,
        CAST(project_sessions.process_group_id AS TEXT)
    ),
    COALESCE(project_sessions.process_started_at, project_sessions.created_at),
    NULL,
    waves.repo,
    waves.repo,
    waves.name,
    NULL,
    NULL,
    project_sessions.project_id,
    NULL,
    COALESCE(
        project_sessions.process_provider,
        project_sessions.provider,
        project_sessions.agent
    ),
    NULL,
    'headless',
    'prompt_only',
    NULL,
    'running',
    '',
    '',
    NULL,
    project_sessions.process_provider_session_id,
    NULL,
    0,
    0,
    runs.id,
    runs.home_id,
    NULL,
    CASE runs.state
        WHEN 'reserved' THEN 'starting'
        WHEN 'active' THEN 'live'
        ELSE 'stopping'
    END,
    CASE
        WHEN project_sessions.process_tmux_name IS NOT NULL THEN 'tmux'
        ELSE 'process_group'
    END,
    COALESCE(
        project_sessions.process_tmux_name,
        CAST(project_sessions.process_group_id AS TEXT)
    ),
    COALESCE(
        project_sessions.process_provider_session_id,
        project_sessions.provider_session_id
    ),
    NULL,
    NULL
FROM runs
JOIN project_sessions
  ON runs.source_kind = 'project'
 AND runs.source_id = project_sessions.id
 AND runs.lease_generation = project_sessions.process_generation
JOIN waves ON waves.id = project_sessions.wave_id
WHERE runs.state IN ('reserved', 'active', 'stopping')
  AND (
      project_sessions.process_tmux_name IS NOT NULL
      OR project_sessions.process_group_id IS NOT NULL
  );

INSERT INTO agent_launches (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, project, task, provider, model, surface, capture_status,
    incomplete_reason, outcome, artifact_dir, conversation_path,
    provider_events_path, provider_session_id, provider_session_path,
    conversation_event_count, conversation_bytes, product_run_id, home_id,
    account_id, launch_state, containment_kind, containment_id, resume_token,
    opaque_epoch_id, opaque_basis_rev
)
SELECT
    'launch_' || lower(hex(randomblob(16))),
    runs.id,
    COALESCE(
        task_sessions.process_tmux_name,
        CAST(task_sessions.process_group_id AS TEXT)
    ),
    COALESCE(task_sessions.process_started_at, task_sessions.created_at),
    NULL,
    waves.repo,
    task_sessions.worktree,
    waves.name,
    NULL,
    NULL,
    task_sessions.project_id,
    task_sessions.issue_identifier,
    COALESCE(
        task_sessions.process_provider,
        task_sessions.provider,
        task_sessions.agent
    ),
    NULL,
    'headless',
    'prompt_only',
    NULL,
    'running',
    '',
    '',
    NULL,
    task_sessions.process_provider_session_id,
    NULL,
    0,
    0,
    runs.id,
    runs.home_id,
    NULL,
    CASE runs.state
        WHEN 'reserved' THEN 'starting'
        WHEN 'active' THEN 'live'
        ELSE 'stopping'
    END,
    CASE
        WHEN task_sessions.process_tmux_name IS NOT NULL THEN 'tmux'
        ELSE 'process_group'
    END,
    COALESCE(
        task_sessions.process_tmux_name,
        CAST(task_sessions.process_group_id AS TEXT)
    ),
    COALESCE(
        task_sessions.process_provider_session_id,
        task_sessions.provider_session_id
    ),
    NULL,
    NULL
FROM runs
JOIN task_sessions
  ON runs.source_kind = 'task'
 AND runs.source_id = task_sessions.id
 AND runs.lease_generation = task_sessions.process_generation
JOIN waves ON waves.id = task_sessions.wave_id
WHERE runs.state IN ('reserved', 'active', 'stopping')
  AND (
      task_sessions.process_tmux_name IS NOT NULL
      OR task_sessions.process_group_id IS NOT NULL
  );

-- A pre-normalization row without a containable process is not safe to call an
-- active Run. Fence it for Session-owned recovery rather than fabricating a
-- Launch or leaving product control apparently live.
UPDATE runs
SET state = 'stopping'
WHERE source_kind IN ('project', 'task')
  AND state IN ('reserved', 'active')
  AND NOT EXISTS (
      SELECT 1 FROM agent_launches
      WHERE agent_launches.product_run_id = runs.id
  );

CREATE UNIQUE INDEX idx_agent_launches_one_control_live
    ON agent_launches(product_run_id)
    WHERE launch_state IN ('starting', 'live', 'stopping');
CREATE INDEX idx_agent_launches_attention
    ON agent_launches(attention_kind, attention_work_kind, attention_work_id, attention_at)
    WHERE attention_kind IS NOT NULL;

CREATE UNIQUE INDEX idx_homes_route ON homes(route);

CREATE TABLE work_flow_positions (
    epoch_id TEXT PRIMARY KEY REFERENCES epochs(id) ON DELETE CASCADE,
    flow TEXT NOT NULL CHECK (length(trim(flow)) > 0),
    step TEXT NOT NULL CHECK (length(trim(step)) > 0),
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    interactive INTEGER NOT NULL CHECK (interactive IN (0, 1)),
    updated_at INTEGER NOT NULL
);

CREATE TABLE done_proposals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    basis_rev INTEGER NOT NULL,
    proposed_at INTEGER NOT NULL,
    UNIQUE (run_id, epoch_id, basis_rev),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);
