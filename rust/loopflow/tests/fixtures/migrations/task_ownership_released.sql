-- Ownership projection of the 0.13.9 released tasks/Workflow schema.
-- The candidate reads this frontier without applying its local-planning draft.
CREATE TABLE tasks (id TEXT PRIMARY KEY, worktree TEXT, abandoned_at INTEGER);
CREATE TABLE task_workflows (task_id TEXT PRIMARY KEY, node TEXT, edge INTEGER);
INSERT INTO tasks VALUES ('running', '/repo.running', NULL);
INSERT INTO tasks VALUES ('unknown', '/repo.unknown', NULL);
INSERT INTO tasks VALUES ('completed', '/repo.completed', NULL);
INSERT INTO tasks VALUES ('abandoned', '/repo.abandoned', 1);
INSERT INTO tasks VALUES ('unplaced', NULL, NULL);
INSERT INTO task_workflows VALUES ('running', 'review', 2);
INSERT INTO task_workflows VALUES ('completed', 'end', NULL);
