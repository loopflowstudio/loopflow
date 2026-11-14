-- Task checkout at the released frontier, before shell receipts and retirement.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_workspace','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at)
VALUES('task_workspace','repo_workspace','SHELL-1',1,'/repo.task','completed',1,'2026-11-03T00:00:00Z');
