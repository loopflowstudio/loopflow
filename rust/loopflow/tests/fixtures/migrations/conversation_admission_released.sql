INSERT INTO waves(id,name,repo,created_at) VALUES('wave','retained','/repo',1);
INSERT INTO projects(id,wave_id,created_at) VALUES('project','wave',1);
INSERT INTO tasks(id,project_id,issue_identifier,created_at) VALUES('started','project','T-1',1);
INSERT INTO tasks(id,project_id,issue_identifier,created_at) VALUES('backlog','project','T-2',1);
INSERT INTO agent_sessions(id,title,title_source,interactive,created_at,repo,task_id,wave_id,work_source,cwd,input_published)
VALUES('history','Retained conversation','human',1,1,'/repo','started','wave','declared','/repo/task',1);
INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
VALUES('history','captured','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',2,'{"artifact_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}');
UPDATE agent_sessions SET current_capture=last_insert_rowid() WHERE id='history';
UPDATE tasks SET primary_session_id='history' WHERE id='started';
