-- name: restore_chapter_snapshot_input
-- id: e2e45d978cf448e5b792adf8f47e5c2e
-- depends_on: pm_project_evidence

-- Normalization consumed the snapshot before chapter conversion. Reconstruct
-- only its reader input from the authoritative normalized rows; no runtime
-- writer uses this table. Per-entity observation ages remain in their owners.
CREATE TABLE pm_snapshots (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);
INSERT INTO pm_snapshots
SELECT s.wave_id,s.provider,s.initiative,s.synced_at,
    json_object('projects',json(COALESCE((
        SELECT json_group_array(json(body)) FROM (
            SELECT p.body FROM pm_wave_projects m
            JOIN pm_projects p ON p.id=m.project_id AND p.provider=s.provider
                AND p.repo=w.repo
            WHERE m.wave_id=s.wave_id ORDER BY m.position
        )
    ),'[]')),'items',json('[]'))
FROM pm_wave_sync s JOIN waves w ON w.id=s.wave_id;
