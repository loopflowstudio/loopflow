-- name: home_runtime_generation
-- id: e78006f90f82abb97da758d12e5f246c
-- depends_on:

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
