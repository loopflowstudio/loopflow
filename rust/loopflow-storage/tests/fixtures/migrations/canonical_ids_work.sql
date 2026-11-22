-- Populated released Work: spelling changes must preserve the whole association.
INSERT INTO waves(id,name,repo,created_at)
VALUES('DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD','Case-Sensitive-Wave','/Case/Repo',1);
INSERT INTO projects(id,wave_id,external_project_id,project_name,planning_initiatives,planning_teams,created_at,project_slug,description,updated_at)
VALUES('PROJ_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD',
'EEEEEEEE-EEEE-4EEE-8EEE-EEEEEEEEEEEE','Case-Sensitive-Project',
'["{AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA}"]','["URN:UUID:BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"]',1,'case-sensitive-project','Case project',1);
UPDATE waves SET current_project_id='PROJ_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';
INSERT INTO machines(id,route,created_at,observed_at)
VALUES('HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','Case-Sensitive-Route',1,2);
INSERT INTO repository_plans(id,repo,selected)
VALUES('REPO_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','/Case/Repo',1);
INSERT INTO tasks(id,repository_id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,workspace_slug,worktree,branch,base_commit,
checkout_machine_id,admission_authority,created_at,updated_at)
VALUES('TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','REPO_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','PROJ_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
'FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF','lOo-506','Original title','Original description','','/Case/Repo/Task','Case/Branch','Case-Sensitive-Hash',
'HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','{"machine_id":"HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","repository_id":"REPO_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}',1,1);
INSERT INTO task_admissions(task_id,machine_id)
VALUES('TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA');
UPDATE agent_sessions SET task_id='TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',wave_id='DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD',
provider_thread='AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA';
UPDATE tasks SET primary_session_id='SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';
INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at,linear_attachment_id)
VALUES('PR_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,'Case-Slug','Case/Branch','Case-Sensitive-Hash',1,2,'AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA');
INSERT INTO task_workflows(task_id,graph,node,edge,lf_process_id,updated_at)
VALUES('TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','{"name":"Case-Flow","nodes":[],"edges":[{"from":"start","to":"end","flow":"Case-Flow"}]}','start',0,'BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB',2);
INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,lf_process_id,note,at)
VALUES('TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','Case-Flow','chose','start','end',0,'BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB','Case-Sensitive note',2);
INSERT INTO task_events(task_id,kind_json,created_at)
VALUES('TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','{"kind":"steer","steer_id":"AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"}',2);
INSERT INTO tool_responses(id,work_kind,work_id,request_id,choice,responded_at)
VALUES('RESPONSE_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','task','TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA','Keep Case',2);
INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES
('AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA','tasks','{"edit":{"target":"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","change":{"field":"task_title","value":"Original title"}},"clock":1,"parents":[]}'),
('BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB','tasks','{"edit":{"target":"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","change":{"field":"task_title","value":"Retained competing title"}},"clock":2,"parents":["AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"]}');
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload)
VALUES('SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','captured','run_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
'TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD',1,
'{"artifact_key":"run_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","unknown_id":"DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD"}');
UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events WHERE kind='captured');
