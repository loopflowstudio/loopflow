-- depends_on: session_ownership

-- Planning facts have repository/provider identity, independent of execution.
CREATE TABLE pm_projects (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY(repo, provider, id)
);
CREATE TABLE pm_items (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    identifier TEXT NOT NULL COLLATE NOCASE,
    project_id TEXT,
    observed_at INTEGER NOT NULL,
    needs_refresh INTEGER NOT NULL DEFAULT 0 CHECK(needs_refresh IN (0,1)),
    body TEXT NOT NULL,
    PRIMARY KEY(repo, provider, id)
);
CREATE INDEX pm_items_identifier ON pm_items(repo, provider, identifier);
CREATE INDEX pm_items_project ON pm_items(repo, provider, project_id);
CREATE TABLE pm_wave_sync (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL
);
CREATE TABLE pm_wave_projects (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(wave_id, project_id)
);

-- Overlapping historical snapshots converge on their latest observation.
-- An unreadable payload is not an empty plan to discard during migration.
CREATE TEMP TABLE validate_pm_snapshots(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO validate_pm_snapshots
SELECT COALESCE(json_type(payload,'$.projects')='array' AND json_type(payload,'$.items')='array',0)
FROM pm_snapshots;
DROP TABLE validate_pm_snapshots;

INSERT INTO pm_projects(repo, provider, id, observed_at, body)
SELECT w.repo, s.provider, json_extract(p.value, '$.id'), s.synced_at, p.value
FROM pm_snapshots s JOIN waves w ON w.id=s.wave_id,
     json_each(s.payload, '$.projects') p
WHERE true ORDER BY s.synced_at, s.wave_id
ON CONFLICT(repo, provider, id) DO UPDATE SET
    observed_at=excluded.observed_at, body=excluded.body;
INSERT INTO pm_items(repo, provider, id, identifier, project_id, observed_at, body)
SELECT w.repo, s.provider, json_extract(i.value, '$.id'),
       json_extract(i.value, '$.identifier'), json_extract(i.value, '$.project_id'),
       s.synced_at, i.value
FROM pm_snapshots s JOIN waves w ON w.id=s.wave_id,
     json_each(s.payload, '$.items') i
WHERE true ORDER BY s.synced_at, s.wave_id
ON CONFLICT(repo, provider, id) DO UPDATE SET
    identifier=excluded.identifier, project_id=excluded.project_id,
    observed_at=excluded.observed_at, body=excluded.body;
INSERT INTO pm_wave_sync SELECT wave_id, provider, initiative, synced_at FROM pm_snapshots;
INSERT INTO pm_wave_projects
SELECT s.wave_id, json_extract(p.value, '$.id'), CAST(p.key AS INTEGER)
FROM pm_snapshots s, json_each(s.payload, '$.projects') p;
DROP TABLE pm_snapshots;
