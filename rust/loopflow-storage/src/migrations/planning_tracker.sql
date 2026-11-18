-- The tracker each repository plans through, as its last Git exchange or
-- tracker drain read it from `.lf/config.yaml`. The planning writer journals
-- a record for Git peers only while its repository's tracker is `git`; NULL
-- means not yet seen, and the first Git exchange seeds the whole plan.
ALTER TABLE repository_plans ADD COLUMN planning_tracker TEXT
    CHECK (planning_tracker IN ('git', 'linear', 'asana'));

-- A repository that has exchanged through Git keeps journaling; a switched one
-- records its tracker at its next drain.
UPDATE repository_plans SET planning_tracker = 'git'
WHERE selected = 1 AND repo IN (SELECT repo FROM planning_destinations);

-- Journal changes no peer holds and no Git destination will publish: unrouted
-- changes whose record is gone or belongs to a repository that never bound one.
-- Routed changes stay; a peer holds them.
CREATE TEMP TABLE unpublished_planning_changes(id TEXT PRIMARY KEY);
INSERT INTO unpublished_planning_changes(id)
SELECT id FROM (
    SELECT c.id, CASE c.record_table
        WHEN 'waves' THEN (SELECT repo FROM waves WHERE id = c.object_id)
        WHEN 'projects' THEN (SELECT w.repo FROM projects p JOIN waves w ON w.id = p.wave_id
            WHERE p.id = c.object_id)
        WHEN 'tasks' THEN (SELECT r.repo FROM tasks t JOIN repository_plans r ON r.id = t.repository_id
            WHERE t.id = c.object_id)
        WHEN 'task_comments' THEN (SELECT r.repo FROM task_comments m JOIN tasks t ON t.id = m.task_id
            JOIN repository_plans r ON r.id = t.repository_id WHERE m.id = c.object_id)
    END AS repo
    FROM planning_peer_changes c
    WHERE NOT EXISTS (SELECT 1 FROM planning_peer_routes r WHERE r.mutation_id = c.id)
)
WHERE repo IS NULL OR repo NOT IN (SELECT repo FROM planning_destinations);
DELETE FROM planning_peer_observed WHERE id IN (SELECT id FROM unpublished_planning_changes);
DELETE FROM planning_peer_changes WHERE id IN (SELECT id FROM unpublished_planning_changes);
DROP TABLE unpublished_planning_changes;
