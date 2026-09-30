-- name: project_status_chapters
-- id: 9bea0388aa854e46a9db2d1e5c86ddfb
-- depends_on: one_flow_driver, restore_chapter_snapshot_input

-- Retain only the identity evidence needed for one-time provider adoption.
-- NULL: converted/new; 1: old current; 0: old noncurrent; -1: no receipt.
-- The marker is cleared after confirmed provider conversion. It owns no plan.
ALTER TABLE projects ADD COLUMN status TEXT NOT NULL DEFAULT 'started'
    CHECK (status IN ('backlog','planned','started','paused','completed','canceled'));
ALTER TABLE projects ADD COLUMN flow TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN legacy_current INTEGER
    CHECK (legacy_current IN (-1, 0, 1));
-- Planning-only Projects may have been observed before local Task allocation.
-- Preserve their exact provider identities so the first refresh can convert them.
INSERT INTO projects (id,wave_id,external_project_id,created_at,project_slug,
    project_name,project_prompt_context,pm_snapshot_synced_at,updated_at)
SELECT 'project_adoption_' || s.wave_id || '_' || json_extract(p.value,'$.id'),
    s.wave_id,json_extract(p.value,'$.id'),s.synced_at,
    json_extract(p.value,'$.slug'),json_extract(p.value,'$.name'),'',s.synced_at,s.synced_at
FROM pm_snapshots s, json_each(s.payload,'$.projects') p
WHERE NOT EXISTS (SELECT 1 FROM projects
    WHERE external_project_id=json_extract(p.value,'$.id'));
INSERT INTO projects (id,wave_id,external_project_id,created_at,project_slug,
    project_name,project_prompt_context,pm_snapshot_synced_at,updated_at)
SELECT 'project_adoption_' || c.wave_id || '_' || c.project_id,c.wave_id,c.project_id,
    COALESCE(json_extract(c.receipt,'$.created_at'),0),c.chapter_id,c.chapter_id,'',0,
    COALESCE(json_extract(c.receipt,'$.created_at'),0)
FROM wave_chapters c
WHERE NOT EXISTS (SELECT 1 FROM projects WHERE external_project_id=c.project_id);
UPDATE projects SET legacy_current = COALESCE((
    SELECT current FROM wave_chapters
    WHERE wave_chapters.wave_id = projects.wave_id
      AND wave_chapters.project_id = projects.external_project_id
), -1);
UPDATE projects SET flow = COALESCE((
    SELECT json_extract(p.value,'$.flows.recommended')
    FROM pm_snapshots s, json_each(s.payload,'$.projects') p
    WHERE s.wave_id=projects.wave_id
      AND json_extract(p.value,'$.id')=projects.external_project_id
), '');
-- Old cached snapshots must decode before their first network refresh, too.
UPDATE pm_snapshots SET payload = json_set(payload, '$.projects', json(COALESCE((
    SELECT json_group_array(json_remove(json_set(p.value,
        '$.flow',COALESCE(json_extract(p.value,'$.flows.recommended'),''),
        '$.status', CASE
            WHEN (SELECT current FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id'))=1 THEN 'started'
            WHEN EXISTS (SELECT 1 FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id')
                AND json_extract(c.receipt,'$.phase')='complete') THEN 'completed'
            WHEN EXISTS (SELECT 1 FROM wave_chapters c WHERE c.project_id=json_extract(p.value,'$.id')) THEN 'planned'
            WHEN json_array_length(pm_snapshots.payload,'$.projects')=1 THEN 'started'
            ELSE 'planned' END
    ), '$.flows'))
    FROM json_each(pm_snapshots.payload,'$.projects') p
), '[]')));
UPDATE projects SET status = COALESCE((
    SELECT json_extract(p.value,'$.status')
    FROM pm_snapshots s, json_each(s.payload,'$.projects') p
    WHERE s.wave_id=projects.wave_id AND json_extract(p.value,'$.id')=projects.external_project_id
), CASE legacy_current WHEN 1 THEN 'started' ELSE 'planned' END);
DROP TABLE wave_chapters;
