-- Released 0.14.3 Work. Started is independent of pause, workflow and Sessions.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('11111111-1111-1111-1111-111111111111','Product','/repo',1),('22222222-2222-2222-2222-222222222222','Empty','/repo',2);
INSERT INTO projects(id,wave_id,created_at,project_name,status) VALUES('proj_11111111111111111111111111111111','11111111-1111-1111-1111-111111111111',1,'Chapter','started');
INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at,started_at,paused,planning_completed,planning_rank)
VALUES
('task_11111111111111111111111111111111','proj_11111111111111111111111111111111','FIX-1','Paused',1,1,NULL,1,0,1),
('task_22222222222222222222222222222222','proj_11111111111111111111111111111111','FIX-2','Idle',1,1,NULL,0,0,2),
('task_33333333333333333333333333333333','proj_11111111111111111111111111111111','FIX-3','Backlog',1,1,NULL,0,0,3),
('task_44444444444444444444444444444444','proj_11111111111111111111111111111111','FIX-4','Done',1,1,NULL,0,1,4);

INSERT INTO task_events(task_id,kind_json,created_at)
SELECT id,'{"kind":"started"}',1 FROM tasks WHERE issue_identifier IN ('FIX-1','FIX-2','FIX-4');
UPDATE tasks SET started_at=1 WHERE issue_identifier IN ('FIX-1','FIX-2','FIX-4');
UPDATE tasks SET issue_description='',workspace_slug='',branch='',base_commit='';
