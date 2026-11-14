-- Current Work for selection readers; the released frontier remains in migrations/saved_selection_work.sql.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('11111111-1111-1111-1111-111111111111','Product','/repo',1),('22222222-2222-2222-2222-222222222222','Empty','/repo',2);
INSERT INTO projects(id,wave_id,created_at,project_name,status) VALUES('proj_11111111111111111111111111111111','11111111-1111-1111-1111-111111111111',1,'Chapter','started');
INSERT INTO tasks(repository_id,id,project_id,issue_identifier,issue_title,created_at,updated_at,planning_completed,planning_rank,worktree)
VALUES
('repo_11111111111111111111111111111111','task_11111111111111111111111111111111','proj_11111111111111111111111111111111','FIX-1','Started, no checkout',1,1,0,1,NULL),
('repo_11111111111111111111111111111111','task_22222222222222222222222222222222','proj_11111111111111111111111111111111','FIX-2','Idle',1,1,0,2,'/repo/fix-2'),
('repo_11111111111111111111111111111111','task_33333333333333333333333333333333','proj_11111111111111111111111111111111','FIX-3','Backlog',1,1,0,3,NULL),
('repo_11111111111111111111111111111111','task_44444444444444444444444444444444','proj_11111111111111111111111111111111','FIX-4','Done',1,1,1,4,NULL);

INSERT INTO task_events(task_id,kind_json,created_at)
SELECT id,'{"kind":"started"}',1 FROM tasks WHERE issue_identifier IN ('FIX-1','FIX-2','FIX-4');
INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
SELECT 'own_'||substr(id,6),id,'recorded',NULL,NULL,1,1000,1 FROM tasks WHERE issue_identifier IN ('FIX-1','FIX-2','FIX-4');
UPDATE tasks SET issue_description='',workspace_slug='',branch='',base_commit='';
