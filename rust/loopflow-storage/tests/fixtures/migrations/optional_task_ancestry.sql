INSERT INTO repository_plans(id,repo,selected) VALUES('repo_retained','/retained',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-retained','inbox','/retained',1),('wave-unselected','unselected','/retained',1);
INSERT INTO projects(id,wave_id,created_at,workflow) VALUES('proj_current','wave-retained',1,'code'),('proj_history','wave-retained',1,'research'),('proj_unselected','wave-unselected',1,'feature');
UPDATE waves SET current_project_id='proj_current' WHERE id='wave-retained';
INSERT INTO tasks(id,project_id,issue_identifier,created_at,worktree,branch,base_commit) VALUES('task_retained','proj_current','OLD-1',1,'/retained/task','branch','base');
INSERT INTO task_creation_intents(task_id,project_id,title,description,export_attempted,export_json) VALUES('task_retained','proj_current','Original','Original brief',1,'{"id":"reserved-issue"}');
INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('retained-session','Review','human',1,1,'/retained/task','task_retained','wave-retained');
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload) VALUES('retained-session','captured','old-capture','task_retained','wave-retained',1,'{}');
INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES('task_retained','{"name":"retained-graph"}','review',1);
