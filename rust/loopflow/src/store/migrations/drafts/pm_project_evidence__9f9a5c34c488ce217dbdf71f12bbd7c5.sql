-- name: pm_project_evidence
-- id: 9f9a5c34c488ce217dbdf71f12bbd7c5
-- depends_on: normalize_pm_planning

-- These are planning evidence, independent of execution and acquisition age.
ALTER TABLE pm_projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1));
ALTER TABLE pm_projects ADD COLUMN membership_unresolved INTEGER NOT NULL DEFAULT 0 CHECK(membership_unresolved IN (0,1));

-- Completed chapter operations recorded successful predecessor archival before
-- marking completion. Preserve those receipts across the planning migration.
INSERT INTO pm_projects(repo,provider,id,observed_at,body,archived)
SELECT w.repo,s.provider,json_extract(p.value,'$.id'),json_extract(c.receipt,'$.created_at'),p.value,1
FROM wave_chapters c JOIN waves w ON w.id=c.wave_id
JOIN pm_wave_sync s ON s.wave_id=c.wave_id, json_each(c.receipt,'$.predecessors') p
WHERE json_extract(c.receipt,'$.phase')='complete'
ON CONFLICT(repo,provider,id) DO UPDATE SET archived=1;
