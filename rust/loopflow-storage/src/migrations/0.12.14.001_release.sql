-- draft: pr_landings
CREATE TABLE pr_landings (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    worktree TEXT NOT NULL,
    branch TEXT NOT NULL,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    requested_head_sha TEXT NOT NULL,
    observed_head_sha TEXT NOT NULL,
    merge_commit TEXT,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    state TEXT NOT NULL CHECK (state IN ('watching', 'repairing', 'merged', 'closed', 'blocked')),
    generation INTEGER NOT NULL CHECK (generation > 0),
    supervisor_placement TEXT CHECK (supervisor_placement IN ('local', 'home')),
    supervisor_home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT,
    supervisor_process_id INTEGER,
    supervisor_heartbeat_at INTEGER,
    repair_count INTEGER NOT NULL DEFAULT 0 CHECK (repair_count >= 0),
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK ((supervisor_placement IS NULL) = (supervisor_process_id IS NULL)),
    CHECK ((supervisor_placement IS NULL) = (supervisor_heartbeat_at IS NULL)),
    CHECK (supervisor_placement = 'home' OR supervisor_home_id IS NULL),
    CHECK (supervisor_placement != 'home' OR supervisor_home_id IS NOT NULL),
    CHECK ((state = 'blocked') = (blocked_reason IS NOT NULL)),
    CHECK (state != 'merged' OR merge_commit IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (task_id IS NOT NULL OR (after_merge IS NULL AND next_slug IS NULL))
);

CREATE UNIQUE INDEX idx_pr_landings_active
    ON pr_landings(repo, pr_number)
    WHERE state IN ('watching', 'repairing');
CREATE INDEX idx_pr_landings_supervisor
    ON pr_landings(state, supervisor_heartbeat_at);

CREATE TABLE ci_incidents_next (
    identity TEXT PRIMARY KEY,
    landing_id TEXT REFERENCES pr_landings(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    pr_id TEXT REFERENCES task_prs(id) ON DELETE SET NULL,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha TEXT NOT NULL,
    failure_set_json TEXT NOT NULL,
    provider_completed_at INTEGER,
    poll_observed_at INTEGER,
    webhook_received_at INTEGER,
    claimed_landing_generation INTEGER,
    responded_at INTEGER,
    green_at INTEGER,
    merged_at INTEGER,
    blocked_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    repaired_head_sha TEXT,
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL)),
    CHECK (landing_id IS NULL OR claimed_landing_generation IS NULL OR claimed_landing_generation > 0)
);

INSERT INTO ci_incidents_next (
    identity, landing_id, task_id, pr_id, repo, pr_number,
    failed_head_sha, failure_set_json, provider_completed_at,
    poll_observed_at, webhook_received_at, claimed_landing_generation,
    responded_at, green_at, merged_at, blocked_at, blocked_reason,
    created_at, updated_at, repaired_head_sha
)
SELECT identity, NULL, task_id, pr_id, repo, pr_number,
       failed_head_sha, failure_set_json, provider_completed_at,
       poll_observed_at, webhook_received_at, NULL,
       responded_at, green_at, merged_at, blocked_at, blocked_reason,
       created_at, updated_at, repaired_head_sha
FROM ci_incidents;

DROP TABLE ci_incidents;
ALTER TABLE ci_incidents_next RENAME TO ci_incidents;
CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_pr ON ci_incidents(pr_id, created_at);
CREATE INDEX idx_ci_incidents_landing ON ci_incidents(landing_id, created_at);
CREATE INDEX idx_ci_incidents_open ON ci_incidents(green_at, merged_at, updated_at);

-- draft: run_identity
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
