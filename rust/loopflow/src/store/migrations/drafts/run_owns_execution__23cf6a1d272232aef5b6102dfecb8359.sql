-- name: run_owns_execution
-- id: 23cf6a1d272232aef5b6102dfecb8359
-- depends_on: explicit_pr_merge_requests

-- Collapse runner authority and containment onto Run. The trace row records
-- one provider invocation and may point at its supervisor, but that relation
-- is provenance only.

-- Production migration wraps the full set with foreign keys disabled. Keep
-- this migration correct when applied directly by validation fixtures too,
-- because invocation ids and their Turn references change together below.
PRAGMA foreign_keys = OFF;

ALTER TABLE runs ADD COLUMN containment_kind TEXT CHECK (
    containment_kind IN ('process_group', 'tmux')
);
ALTER TABLE runs ADD COLUMN containment_id TEXT;
ALTER TABLE runs ADD COLUMN cwd TEXT;
ALTER TABLE runs ADD COLUMN started_at INTEGER;

-- Preserve the strongest containment already recorded by the former control
-- Launch. Historical ended Runs retain it; never-started reservations remain
-- empty.
UPDATE runs
SET containment_kind = (
        SELECT containment_kind FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    containment_id = (
        SELECT containment_id FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    cwd = (
        SELECT worktree FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    started_at = (
        SELECT started_at FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    )
WHERE EXISTS (
    SELECT 1 FROM agent_launches
    WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
);

-- An imported controller without containable evidence cannot remain an
-- execution authority under the reduced model.
UPDATE runs
SET state = 'ended',
    ended_at = COALESCE(ended_at, unixepoch()),
    stop_reason = COALESCE(stop_reason, 'migration: missing Run containment'),
    containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state IN ('active', 'stopping')
  AND (
      containment_kind IS NULL OR containment_id IS NULL OR cwd IS NULL
      OR started_at IS NULL
  );

-- A reservation has not acquired containment yet. Old control Launch rows may
-- have recorded a proposed containment while the Run was still reserved; do
-- not promote that proposal into acquired Run state.
UPDATE runs
SET containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state = 'reserved';

-- Historical ended rows were not constrained as one checked group. Discard
-- partial evidence rather than importing an impossible containment identity.
UPDATE runs
SET containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state = 'ended'
  AND (
      containment_kind IS NULL OR containment_id IS NULL OR cwd IS NULL
      OR started_at IS NULL
  );

UPDATE agent_launches
SET launch_state = 'ended',
    ended_at = COALESCE(ended_at, unixepoch()),
    outcome = CASE WHEN outcome = 'running' THEN 'failed' ELSE outcome END,
    handback_state = COALESCE(handback_state, 'unknown')
WHERE product_run_id IN (
    SELECT id FROM runs
    WHERE state = 'ended'
)
  AND ended_at IS NULL;

UPDATE agent_turns
SET status = 'failed',
    ended_at = COALESCE(ended_at, unixepoch())
WHERE status = 'running'
  AND launch_id IN (
      SELECT agent_launches.id
      FROM agent_launches
      JOIN runs ON runs.id = agent_launches.product_run_id
      WHERE runs.state = 'ended'
  );

CREATE TRIGGER runs_execution_shape_insert
BEFORE INSERT ON runs
BEGIN
    SELECT RAISE(ABORT, 'invalid Run execution shape')
    WHERE NOT (
        (NEW.state = 'reserved'
         AND NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
         AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
        OR
        (NEW.state IN ('active', 'stopping')
         AND NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
         AND length(trim(NEW.containment_id)) > 0
         AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
         AND NEW.started_at IS NOT NULL)
        OR
        (NEW.state = 'ended'
         AND (
             (NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
              AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
             OR
             (NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
              AND length(trim(NEW.containment_id)) > 0
              AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
              AND NEW.started_at IS NOT NULL)
         ))
    );
END;

CREATE TRIGGER runs_execution_shape_update
BEFORE UPDATE ON runs
BEGIN
    SELECT RAISE(ABORT, 'invalid Run execution shape')
    WHERE NOT (
        (NEW.state = 'reserved'
         AND NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
         AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
        OR
        (NEW.state IN ('active', 'stopping')
         AND NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
         AND length(trim(NEW.containment_id)) > 0
         AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
         AND NEW.started_at IS NOT NULL)
        OR
        (NEW.state = 'ended'
         AND (
             (NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
              AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
             OR
             (NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
              AND length(trim(NEW.containment_id)) > 0
              AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
              AND NEW.started_at IS NOT NULL)
         ))
    );
    SELECT RAISE(ABORT, 'Run containment is immutable once acquired')
    WHERE OLD.containment_kind IS NOT NULL
      AND (
          NEW.containment_kind IS NOT OLD.containment_kind
          OR NEW.containment_id IS NOT OLD.containment_id
          OR NEW.cwd IS NOT OLD.cwd
          OR NEW.started_at IS NOT OLD.started_at
      );
END;

DROP INDEX idx_agent_launches_one_control_live;
DROP INDEX idx_agent_launches_attention;
DROP INDEX idx_agent_launches_run;
DROP INDEX idx_agent_launches_process;
DROP INDEX idx_agent_launches_wave;
DROP INDEX idx_agent_launches_project;
DROP INDEX idx_agent_launches_task;

ALTER TABLE agent_launches RENAME TO agent_invocations;
ALTER TABLE agent_invocations RENAME COLUMN product_run_id TO supervising_run_id;
ALTER TABLE agent_invocations DROP COLUMN home_id;
ALTER TABLE agent_invocations DROP COLUMN launch_state;
ALTER TABLE agent_invocations DROP COLUMN containment_kind;
ALTER TABLE agent_invocations DROP COLUMN containment_id;
ALTER TABLE agent_invocations DROP COLUMN opaque_epoch_id;
ALTER TABLE agent_invocations DROP COLUMN opaque_basis_rev;

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
CREATE INDEX idx_agent_invocations_attention
    ON agent_invocations(
        attention_kind, attention_work_kind, attention_work_id, attention_at
    )
    WHERE attention_kind IS NOT NULL;

DROP INDEX idx_agent_turns_launch;
ALTER TABLE agent_turns RENAME COLUMN launch_id TO invocation_id;

-- Legacy invocation ids were minted with the old type prefix. Rewrite every
-- matching trace identity and Turn reference once; unrelated imported ids are
-- untouched.
UPDATE agent_invocations
SET id = 'invocation_' || substr(id, 8)
WHERE id GLOB 'launch_*';
UPDATE agent_turns
SET invocation_id = 'invocation_' || substr(invocation_id, 8)
WHERE invocation_id GLOB 'launch_*';

CREATE INDEX idx_agent_turns_invocation
    ON agent_turns(invocation_id, ordinal);

PRAGMA foreign_keys = ON;
