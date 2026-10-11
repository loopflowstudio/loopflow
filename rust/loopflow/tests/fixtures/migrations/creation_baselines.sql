-- Released creation receipts: the attempted model differs from later local saves.
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-proof','proof','/repo',1);
INSERT INTO projects(id,wave_id,created_at,project_name,project_summary,status)
VALUES('project-proof','wave-proof',1,'Later Project','Later summary','started');
INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at)
VALUES('task-proof','project-proof','PROOF-1','Later Task',1,1);
INSERT INTO project_transitions(wave_id,successor_id,created_at,export_attempted,export_link_attempted,export_error,export_json)
VALUES('wave-proof','project-proof',1,1,1,'Unknown attachment',
'{"id":"project-external","model":{"id":"project-proof","name":"Original Project","summary":"Original summary","workflow":"code","krs":[],"metric_targets":[],"status":"planned","team_ids":["original-team"]},"through":7,"input":{"id":"project-external","teamIds":["original-team"],"name":"Original Project","description":"Original summary","content":"workflow: code","statusId":"original-status","useDefaultTemplate":false},"initiative":"original-initiative","link_id":"original-link"}');
INSERT INTO task_creation_intents(task_id,project_id,title,description,export_attempted,export_error,export_json)
VALUES('task-proof','project-proof','Original Task','Original body',1,'Lost creation reply',
'{"id":"task-external","model":{"id":"task-proof","name":"Original Task","description":"Original body","project_id":"project-external","assignee":null,"due_date":"2026-10-20","state":"unstarted","completed":false,"completed_at":null,"branch_name":"retained-branch"},"through":7,"input":{"id":"task-external","teamId":"original-team","projectId":"project-external","title":"Original Task","description":"Original body","stateId":"original-state","assigneeId":null,"dueDate":"2026-10-20"},"initiative":"original-initiative","link_id":"task-link"}');
UPDATE projects SET external_project_id='project-external' WHERE id='project-proof';
INSERT INTO task_changes(seq,id,task_id,field,value_json,attempted,acknowledged)
VALUES(5,'original-task','task-proof','name','"Original Task"',0,0),
      (6,'acked-task','task-proof','description','"Earlier body"',1,1),
      (8,'later-task','task-proof','name','"Later Task"',0,0);
INSERT INTO project_changes(seq,id,project_id,field,value_json,attempted,acknowledged)
VALUES(5,'original-project','project-proof','name','"Original Project"',0,0),
      (8,'later-project','project-proof','name','"Later Project"',0,0);
INSERT INTO task_changes(seq,id,task_id,field,value_json,base_json,conflict_json,attempted,error)
VALUES(9,'conflicted-task','task-proof','assignee','"local-author"','{"revision":"2026-10-09T00:00:00Z","value":null}',
       '{"revision":"2026-10-10T00:00:00Z","value":"provider-author"}',1,'Retained conflict');
UPDATE sqlite_sequence SET seq=42 WHERE name IN ('task_changes','project_changes');

-- An earlier attempted field write and a save made while its reply was missing.
INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at)
VALUES('task-receipt','project-proof','PROOF-2','Later save',1,1);
INSERT INTO task_changes(seq,id,task_id,field,value_json,base_json,attempted,error)
VALUES(10,'attempted-title','task-receipt','name','"Attempted title"',
       '{"revision":"2026-10-09T00:00:00Z","value":"Provider before"}',1,'Lost reply'),
      (11,'later-title','task-receipt','name','"Later save"',
       '{"revision":"2026-10-09T00:00:00Z","value":"Provider before"}',0,NULL),
      (12,'null-membership','task-receipt','project_id','"project-proof"',
       '{"revision":"2026-10-09T00:00:00Z","value":null}',0,NULL);
