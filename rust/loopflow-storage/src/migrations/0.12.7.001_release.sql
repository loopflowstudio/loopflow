-- draft: home_runtime_generation
CREATE TABLE home_runtime_generations (
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    generation INTEGER NOT NULL CHECK (generation > 0),
    build_version TEXT NOT NULL,
    source_revision TEXT NOT NULL,
    migration_frontier TEXT NOT NULL,
    activated_at INTEGER NOT NULL,
    PRIMARY KEY (home_id, generation)
);

CREATE TABLE home_upgrades (
    id TEXT PRIMARY KEY,
    home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT,
    source_revision TEXT NOT NULL,
    source_identity TEXT NOT NULL,
    migration_authority TEXT NOT NULL
        CHECK (migration_authority IN ('published', 'validation_only')),
    package_version TEXT NOT NULL,
    build_version TEXT,
    latest_known_migration TEXT NOT NULL,
    prior_generation INTEGER NOT NULL CHECK (prior_generation >= 0),
    target_generation INTEGER NOT NULL CHECK (target_generation > prior_generation),
    phase TEXT NOT NULL CHECK (phase IN (
        'planned', 'draining', 'drained', 'migrating', 'restarting',
        'reconciling', 'completed', 'failed', 'rolled_back'
    )),
    keeper_mode TEXT NOT NULL CHECK (keeper_mode IN ('none', 'launchd', 'systemd')),
    cli_binary TEXT,
    cli_target TEXT,
    daemon_binary TEXT,
    daemon_target TEXT,
    app_source TEXT,
    app_target TEXT,
    app_superseded TEXT,
    legacy_app_target TEXT,
    migration_required INTEGER NOT NULL CHECK (migration_required IN (0, 1)),
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    artifacts_activated INTEGER NOT NULL CHECK (artifacts_activated IN (0, 1)),
    migration_applied INTEGER NOT NULL CHECK (migration_applied IN (0, 1)),
    daemon_restarted INTEGER NOT NULL CHECK (daemon_restarted IN (0, 1)),
    drain_timed_out INTEGER NOT NULL CHECK (drain_timed_out IN (0, 1)),
    coordinator_started_at INTEGER NOT NULL,
    recovery_pid INTEGER,
    error TEXT,
    CHECK (
        (cli_binary IS NULL AND cli_target IS NULL
            AND daemon_binary IS NULL AND daemon_target IS NULL)
        OR
        (cli_binary IS NOT NULL AND cli_target IS NOT NULL
            AND daemon_binary IS NOT NULL AND daemon_target IS NOT NULL)
    )
);

CREATE TABLE home_upgrade_work (
    upgrade_id TEXT NOT NULL REFERENCES home_upgrades(id) ON DELETE CASCADE,
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL,
    enabled_before INTEGER NOT NULL CHECK (enabled_before IN (0, 1)),
    prior_run_id TEXT,
    resumed_run_id TEXT,
    containment_kind TEXT CHECK (containment_kind IN ('tmux', 'process_group')),
    containment_id TEXT,
    containment_observation TEXT NOT NULL
        CHECK (containment_observation IN ('absent', 'present', 'unprovable')),
    drain TEXT NOT NULL
        CHECK (drain IN ('pending', 'durable_only', 'interrupted', 'forced', 'failed')),
    reconciliation TEXT NOT NULL
        CHECK (reconciliation IN ('pending', 'resumed', 'skipped', 'failed')),
    error TEXT,
    PRIMARY KEY (upgrade_id, work_kind, work_id),
    CHECK ((containment_kind IS NULL) = (containment_id IS NULL))
);

CREATE INDEX idx_home_upgrades_home_started
    ON home_upgrades(home_id, target_generation DESC, started_at DESC, id DESC);

ALTER TABLE runs ADD COLUMN runtime_generation INTEGER;

CREATE INDEX idx_runs_runtime_generation
    ON runs(home_id, runtime_generation, state);

-- draft: repository_owned_waves
-- A Wave's UUID is durable identity. Its repository and slug are one mutable,
-- repository-scoped locator.
CREATE TABLE waves_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES waves_next(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    UNIQUE (repo, name)
);

INSERT INTO waves_next (
    id, name, repo, created_at, parent_wave_id, promoted_at
)
SELECT id, name, repo, created_at, parent_wave_id, promoted_at
FROM waves;

DROP TABLE waves;
ALTER TABLE waves_next RENAME TO waves;
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);

CREATE TABLE pm_snapshots_next (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);

-- The NOT NULL primary key makes an unmatched legacy projection abort the
-- migration instead of silently disappearing.
INSERT INTO pm_snapshots_next (
    wave_id, provider, initiative, synced_at, payload
)
SELECT (
    SELECT waves.id
    FROM waves
    WHERE waves.repo = pm_snapshots.repo
      AND waves.name = pm_snapshots.wave
), provider, initiative, synced_at, payload
FROM pm_snapshots;

DROP TABLE pm_snapshots;
ALTER TABLE pm_snapshots_next RENAME TO pm_snapshots;

-- draft: task_performance_authority
-- One cutover owns the meaning of absence for lifecycle performance evidence.
-- Historical terminal PRs remain uncovered. Triggers cover every PR created
-- after the migration even when an older installed writer omits new columns.
CREATE TABLE performance_evidence_authority (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    started_at INTEGER NOT NULL
);
INSERT INTO performance_evidence_authority (singleton, started_at)
VALUES (1, unixepoch());

ALTER TABLE runs ADD COLUMN first_material_at INTEGER;

CREATE TRIGGER runs_preserve_first_material
BEFORE UPDATE OF first_material_at ON runs
WHEN OLD.first_material_at IS NOT NULL
 AND NEW.first_material_at IS NOT OLD.first_material_at
BEGIN
    SELECT RAISE(ABORT, 'Run first material evidence is immutable');
END;

ALTER TABLE task_prs ADD COLUMN merged_at INTEGER;
ALTER TABLE task_prs
ADD COLUMN merge_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (merge_tracking_complete IN (0, 1));
ALTER TABLE task_prs
ADD COLUMN repair_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (repair_tracking_complete IN (0, 1));

-- A PR still active at cutover has not reached its merge boundary, so the new
-- writer can cover its future GitHub timestamp. Its repair history may already
-- be partial, so repair tracking intentionally remains incomplete.
UPDATE task_prs
SET merge_tracking_complete = 1
WHERE merge_commit IS NULL AND abandoned_at IS NULL;

CREATE TRIGGER task_prs_enable_performance_tracking
AFTER INSERT ON task_prs
BEGIN
    UPDATE task_prs
    SET merge_tracking_complete = 1,
        repair_tracking_complete = 1
    WHERE id = NEW.id;
END;

CREATE TRIGGER task_prs_preserve_merged_at
BEFORE UPDATE OF merged_at ON task_prs
WHEN OLD.merged_at IS NOT NULL
 AND NEW.merged_at IS NOT OLD.merged_at
BEGIN
    SELECT RAISE(ABORT, 'Task PR merge evidence is immutable');
END;

CREATE TABLE task_pr_repair_incidents (
    task_pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (
        kind IN ('avoidable_rebase_agent', 'manual_git_repair')
    ),
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY (task_pr_id, kind)
);

CREATE TRIGGER task_pr_repair_incidents_require_active_pr
BEFORE INSERT ON task_pr_repair_incidents
WHEN EXISTS (
    SELECT 1
    FROM task_prs
    WHERE id = NEW.task_pr_id
      AND (merge_commit IS NOT NULL OR abandoned_at IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'repair incident requires an active Task PR');
END;

CREATE TRIGGER task_pr_repair_incidents_are_immutable
BEFORE UPDATE ON task_pr_repair_incidents
BEGIN
    SELECT RAISE(ABORT, 'Task PR repair incidents are immutable');
END;

-- draft: work_enablement
ALTER TABLE work_placements
ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1));
