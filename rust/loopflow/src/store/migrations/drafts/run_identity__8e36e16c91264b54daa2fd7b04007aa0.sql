-- name: run_identity
-- id: 8e36e16c91264b54daa2fd7b04007aa0
-- depends_on: pr_landings

CREATE TABLE runs_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    runtime_generation INTEGER,
    state TEXT NOT NULL CHECK (state IN ('reserved', 'active', 'stopping', 'ended')),
    trigger_json TEXT NOT NULL CHECK (json_valid(trigger_json)),
    retry_of TEXT REFERENCES runs_next(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('wave', 'project', 'task', 'migration')),
    source_id TEXT,
    created_at INTEGER NOT NULL,
    ended_at INTEGER,
    stop_reason TEXT,
    containment_kind TEXT CHECK (containment_kind IN ('process_group', 'tmux')),
    containment_id TEXT,
    cwd TEXT,
    started_at INTEGER,
    first_material_at INTEGER,
    CHECK ((state = 'ended') = (ended_at IS NOT NULL))
);

INSERT INTO runs_next (
    id, epoch_id, home_id, runtime_generation, state, trigger_json, retry_of,
    source_kind, source_id, created_at, ended_at, stop_reason,
    containment_kind, containment_id, cwd, started_at, first_material_at
)
SELECT
    id, epoch_id, home_id, runtime_generation, state, trigger_json, retry_of,
    source_kind, source_id, created_at, ended_at, stop_reason,
    containment_kind, containment_id, cwd, started_at, first_material_at
FROM runs;

DROP TABLE runs;
ALTER TABLE runs_next RENAME TO runs;

CREATE UNIQUE INDEX idx_runs_one_active_epoch
    ON runs(epoch_id) WHERE state != 'ended';
CREATE INDEX idx_runs_runtime_generation
    ON runs(home_id, runtime_generation, state);

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

CREATE TRIGGER runs_preserve_first_material
BEFORE UPDATE OF first_material_at ON runs
WHEN OLD.first_material_at IS NOT NULL
 AND NEW.first_material_at IS NOT OLD.first_material_at
BEGIN
    SELECT RAISE(ABORT, 'Run first material evidence is immutable');
END;
