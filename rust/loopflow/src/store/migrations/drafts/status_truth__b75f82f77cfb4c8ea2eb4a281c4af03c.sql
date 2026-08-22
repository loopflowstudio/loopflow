-- name: status_truth
-- id: b75f82f77cfb4c8ea2eb4a281c4af03c
-- depends_on:

CREATE TABLE run_liveness (
    run_id TEXT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    observation TEXT NOT NULL CHECK (
        observation IN ('present', 'absent', 'unprovable')
    ),
    observed_at INTEGER NOT NULL,
    runtime_generation INTEGER NOT NULL CHECK (runtime_generation > 0)
);

CREATE INDEX idx_run_liveness_home_observed
    ON run_liveness(home_id, observed_at DESC);

ALTER TABLE project_events ADD COLUMN run_id TEXT
    REFERENCES runs(id) ON DELETE RESTRICT;

-- Attribute old events only when one Project Run uniquely contains their
-- timestamp. Controller events and overlapping legacy intervals stay unknown.
UPDATE project_events AS event
SET run_id = (
    SELECT run.id
    FROM runs AS run
    JOIN epochs AS epoch ON epoch.id = run.epoch_id
    WHERE epoch.project_id = event.project_id
      AND run.created_at <= event.created_at
      AND COALESCE(run.ended_at, 253402300799) >= event.created_at
    LIMIT 1
)
WHERE json_extract(event.kind_json, '$.kind') IN (
        'started', 'iteration_completed', 'completed', 'failed'
    )
  AND 1 = (
    SELECT COUNT(*)
    FROM runs AS run
    JOIN epochs AS epoch ON epoch.id = run.epoch_id
    WHERE epoch.project_id = event.project_id
      AND run.created_at <= event.created_at
      AND COALESCE(run.ended_at, 253402300799) >= event.created_at
);

CREATE INDEX idx_project_events_run ON project_events(run_id, id)
    WHERE run_id IS NOT NULL;

-- Process disappearance is neither a provider failure nor an interruption.
-- Rebuild the table to widen the stored outcome vocabulary.
CREATE TABLE agent_invocations_next (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave TEXT,
    flow TEXT,
    skill TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    surface TEXT NOT NULL,
    capture_status TEXT NOT NULL CHECK (
        capture_status IN (
            'capturing', 'complete', 'partial', 'prompt_only',
            'pruned', 'interrupted', 'lost'
        )
    ),
    incomplete_reason TEXT,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'completed', 'failed', 'interrupted', 'unknown')
    ),
    artifact_dir TEXT NOT NULL,
    conversation_path TEXT NOT NULL,
    provider_events_path TEXT,
    provider_session_id TEXT,
    provider_session_path TEXT,
    conversation_event_count INTEGER NOT NULL,
    conversation_bytes INTEGER NOT NULL,
    project TEXT,
    task TEXT,
    supervising_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    account_id TEXT,
    resume_token TEXT,
    handback_state TEXT CHECK (
        handback_state IN ('succeeded', 'failed', 'interrupted', 'unknown')
    ),
    answer_ask_id TEXT REFERENCES ask_exchanges(id),
    ask_ready_at INTEGER,
    ask_presented_at INTEGER
);

INSERT INTO agent_invocations_next (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task, supervising_run_id, account_id,
    resume_token, handback_state, answer_ask_id, ask_ready_at, ask_presented_at
)
SELECT
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason,
    outcome, artifact_dir, conversation_path, provider_events_path,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes, project, task, supervising_run_id, account_id,
    resume_token, handback_state, answer_ask_id, ask_ready_at, ask_presented_at
FROM agent_invocations;

DROP TABLE agent_invocations;
ALTER TABLE agent_invocations_next RENAME TO agent_invocations;

CREATE INDEX idx_agent_invocations_run
    ON agent_invocations(run_id, started_at);
CREATE INDEX idx_agent_invocations_process
    ON agent_invocations(process_id, started_at);
CREATE INDEX idx_agent_invocations_wave
    ON agent_invocations(wave, started_at);
CREATE INDEX idx_agent_invocations_project
    ON agent_invocations(project, started_at);
CREATE INDEX idx_agent_invocations_task
    ON agent_invocations(task, started_at);
CREATE INDEX idx_agent_invocations_supervisor
    ON agent_invocations(supervising_run_id, started_at)
    WHERE supervising_run_id IS NOT NULL;
CREATE UNIQUE INDEX idx_agent_invocations_one_live_answer
    ON agent_invocations(answer_ask_id)
    WHERE answer_ask_id IS NOT NULL AND ended_at IS NULL;

-- A recovery-ended Run proves that an open Invocation returned no terminal
-- receipt. Settle only that exact legacy shape; provider failures and explicit
-- handbacks retain their recorded outcomes. Older migrations stored free-form
-- stop reasons, so only typed JSON can prove recovery.
UPDATE agent_invocations AS invocation
SET ended_at = (
        SELECT run.ended_at FROM runs AS run
        WHERE run.id = invocation.supervising_run_id
    ),
    outcome = 'unknown',
    handback_state = 'unknown'
WHERE invocation.ended_at IS NULL
  AND invocation.outcome = 'running'
  AND invocation.handback_state IS NULL
  AND invocation.supervising_run_id IN (
      SELECT run.id FROM runs AS run
      WHERE run.state = 'ended'
        AND run.ended_at IS NOT NULL
        AND json_valid(run.stop_reason)
        AND json_extract(run.stop_reason, '$.kind') = 'recovery'
  );

-- Keep one canonical UUID-shaped id type. Historical eight-hex selectors stay
-- usable as unique prefixes of the migrated row.
CREATE TEMP TABLE legacy_invocation_ids (
    old_id TEXT PRIMARY KEY,
    new_id TEXT NOT NULL UNIQUE
);

INSERT INTO legacy_invocation_ids (old_id, new_id)
SELECT id, id || '000000000000000000000000'
FROM agent_invocations
WHERE id GLOB 'invocation_[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]';

UPDATE agent_turns
SET invocation_id = (
    SELECT new_id FROM legacy_invocation_ids WHERE old_id = invocation_id
)
WHERE invocation_id IN (SELECT old_id FROM legacy_invocation_ids);

UPDATE ask_exchanges
SET origin_invocation_id = (
    SELECT new_id FROM legacy_invocation_ids WHERE old_id = origin_invocation_id
)
WHERE origin_invocation_id IN (SELECT old_id FROM legacy_invocation_ids);

UPDATE ask_exchanges
SET active_invocation_id = (
    SELECT new_id FROM legacy_invocation_ids WHERE old_id = active_invocation_id
)
WHERE active_invocation_id IN (SELECT old_id FROM legacy_invocation_ids);

UPDATE agent_invocations
SET id = (SELECT new_id FROM legacy_invocation_ids WHERE old_id = id)
WHERE id IN (SELECT old_id FROM legacy_invocation_ids);

DROP TABLE legacy_invocation_ids;

-- A retired Wave remains addressable by stable id but no longer owns its old
-- repository locator. Active locator lookup is protected by the partial unique
-- index; tombstones may retain the historical repo/name pair.
CREATE TABLE waves_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES waves_next(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    retired_at INTEGER,
    superseded_by_wave_id TEXT REFERENCES waves_next(id) ON DELETE RESTRICT,
    retirement_reason TEXT,
    CHECK (
        (retired_at IS NULL
         AND superseded_by_wave_id IS NULL
         AND retirement_reason IS NULL)
        OR
        (retired_at IS NOT NULL
         AND superseded_by_wave_id IS NOT NULL
         AND length(trim(retirement_reason)) > 0)
    )
);

INSERT INTO waves_next (
    id, name, repo, created_at, parent_wave_id, promoted_at,
    retired_at, superseded_by_wave_id, retirement_reason
)
SELECT id, name, repo, created_at, parent_wave_id, promoted_at, NULL, NULL, NULL
FROM waves;

DROP TABLE waves;
ALTER TABLE waves_next RENAME TO waves;

CREATE INDEX idx_waves_parent
    ON waves(parent_wave_id) WHERE retired_at IS NULL;
CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo, name) WHERE retired_at IS NULL;
CREATE INDEX idx_waves_superseded_by
    ON waves(superseded_by_wave_id) WHERE superseded_by_wave_id IS NOT NULL;
