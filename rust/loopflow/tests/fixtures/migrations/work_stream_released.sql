-- Released 0.13.13 planning and an already captured Task Workflow.
INSERT INTO waves(id,name,repo,created_at) VALUES('{wave}','product','{repo}',1);
INSERT INTO projects(id,wave_id,created_at,project_name,project_slug,workflow,description,updated_at)
VALUES('{project}','{wave}',1,'Current','current','feature','',1);
INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,workspace_slug,agent,created_at,updated_at)
VALUES('{task}','{project}','PROOF-1','Retained Task','Retained brief','retained','codex',1,1);
INSERT INTO task_workflows(task_id,graph,node,updated_at)
VALUES('{task}','{"name":"feature","nodes":[{"name":"demo","skill":"demo","description":null}],"edges":[{"from":"start","to":"demo","flow":"pursue"},{"from":"demo","to":"end","flow":null}]}','demo',3);
