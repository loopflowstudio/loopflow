INSERT INTO waves(id,name,repo,created_at) VALUES('11111111-1111-4111-8111-111111111111','fixture','/repo',1);
INSERT INTO projects(id,wave_id,created_at) VALUES('project','11111111-1111-4111-8111-111111111111',1);
INSERT INTO tasks(id,project_id,issue_identifier,created_at) VALUES('task','project','T-1',1);
INSERT INTO task_events(id,task_id,kind_json,created_at) VALUES
 (11,'task','{"kind":"steer","author":{"kind":"user"},"text":"Historical 界 direction"}',1),
 (12,'task','{"kind":"interrupt"}',2);
INSERT INTO project_events(id,project_id,kind_json,created_at) VALUES
 (21,'project','{"kind":"task_observed","task_id":"task","task_event_id":11,"event":{"kind":"steer","author":{"kind":"user"},"text":"Historical 界 direction"}}',1),
 (22,'project','{"kind":"steer","author":{"kind":"user"},"text":"Project history"}',1),
 (23,'project','{"kind":"task_observed","task_id":"task","task_event_id":999,"event":{"kind":"steer","author":{"kind":"user"},"text":"Orphan retained source"}}',1);
INSERT INTO task_linear_ingested_comments(task_id,comment_id,ingested_at) VALUES('task','comment@revision',1);
