-- name: finish_planning_integration
-- id: a3b654925d99448384ad2c2ab20f5335
-- depends_on: project_status_chapters, restore_chapter_snapshot_input

-- Transfer chapter conversion into the one planning owner, preserving every
-- other field, revision, archive flag and observation timestamp. Already
-- converted execution-branch bodies need no rewrite.
UPDATE pm_projects AS p SET body=json_remove(json_set(p.body,
    '$.flow',json_extract(s.payload,'$.projects[' || j.key || '].flow'),
    '$.status',json_extract(s.payload,'$.projects[' || j.key || '].status')
), '$.flows')
FROM pm_snapshots s JOIN waves w ON w.id=s.wave_id,
    json_each(s.payload,'$.projects') j
WHERE p.repo=w.repo AND p.provider=s.provider AND p.id=json_extract(j.value,'$.id')
    AND json_type(p.body,'$.status') IS NULL;
-- Archived predecessors are intentionally absent from current membership.
UPDATE pm_projects SET body=json_remove(json_set(body,
    '$.flow',COALESCE(json_extract(body,'$.flows.recommended'),''),
    '$.status','completed'
), '$.flows') WHERE archived=1 AND json_type(body,'$.status') IS NULL;
DROP TABLE pm_snapshots;
DROP TABLE IF EXISTS wave_chapters;
