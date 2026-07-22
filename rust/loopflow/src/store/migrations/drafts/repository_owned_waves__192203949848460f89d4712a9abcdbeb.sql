-- name: repository_owned_waves
-- id: 192203949848460f89d4712a9abcdbeb
-- depends_on:
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
