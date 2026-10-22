INSERT INTO waves(id,name,repo,created_at) VALUES('wave','retained','/before',1);
INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('project','wave','external',1);
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at)
VALUES('task','project','external-task','PROOF-1','/before/task',1);
INSERT INTO processes(id,trace_id,cwd,started_at) VALUES('process','trace','/before/task',1);
