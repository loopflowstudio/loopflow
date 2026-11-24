-- Released receipts, one unconfirmed row of each kind, plus rows the queue drops.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo-proof','/repo',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-proof','proof','/repo',1);
INSERT INTO projects(id,wave_id,external_project_id,created_at,project_name,status)
VALUES('project-bound','wave-proof','linear-project',1,'Bound','started'),
      ('project-new','wave-proof',NULL,1,'New','planned');
INSERT INTO project_transitions(wave_id,successor_id,created_at,local_plan_json,export_attempted,export_link_attempted,export_json)
VALUES('wave-proof','project-new',1,'{}',1,0,
'{"id":"project-new-uuid","baseline":[{"field":"project_name","value":"New"}],"task_state":null,"through":0,"input":{"teamIds":["team"]},"initiative":"initiative-proof","link_id":"link-proof"}');
INSERT INTO tasks(id,repository_id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,created_at,updated_at,planning_completed_at)
VALUES('task-bound','repo-proof','project-bound','linear-issue','PROOF-1','Bound','',1,1,'2026-11-01T00:00:00Z'),
      ('task-new','repo-proof',NULL,NULL,'PROOF-2','New','',1,1,NULL),
      ('task-git','repo-proof',NULL,NULL,'PROOF-3','Git only','',1,1,NULL);
INSERT INTO task_creation_intents(task_id,repository_id,title,description,export_attempted,export_error,export_json)
VALUES('task-new','repo-proof','New','',1,'Lost creation reply',
'{"id":"task-new-uuid","baseline":[{"field":"task_title","value":"New"}],"task_state":"unstarted","through":0,"input":{"teamId":"team","stateId":"state"},"initiative":null,"link_id":"task-link"}'),
      ('task-git','repo-proof','Git only','',0,NULL,NULL);
INSERT INTO task_changes(seq,id,task_id,value_json,base_json,attempted,acknowledged)
VALUES(1,'acked','task-bound','{"field":"task_title","value":"Acked"}',NULL,1,1),
      (2,'attempted-title','task-bound','{"field":"task_title","value":"Sent"}','{"revision":"r1","value":{"field":"task_title","value":"Before"}}',1,0),
      (3,'superseded-title','task-bound','{"field":"task_title","value":"Older"}',NULL,0,0),
      (4,'latest-title','task-bound','{"field":"task_title","value":"Latest"}',NULL,0,0),
      (5,'deleted','task-bound','{"value":true}','{"revision":"r1","value":false}',0,0),
      (6,'git-title','task-git','{"field":"task_title","value":"Git"}',NULL,0,0);
INSERT INTO project_changes(seq,id,project_id,value_json,attempted,acknowledged)
VALUES(1,'project-name','project-bound','{"field":"project_name","value":"Renamed"}',0,0),
      (2,'project-order','project-bound','["task-bound"]',1,0);
INSERT INTO task_state_deliveries(seq,id,task_id,target,base_revision,base_state,attempted,settled)
VALUES(1,'state-done','task-bound','completed','r1','started',0,0);
INSERT INTO task_comments(id,task_id,body,author,created_at)
VALUES('comment-proof','task-bound','first comment','{"kind":"person","name":"Proof"}',2);
INSERT INTO task_comment_deliveries(comment_id,comment_json,acknowledged)
VALUES('comment-proof','{"id":"comment-proof","body":"first comment","author":{"kind":"person","name":"Proof"},"created_at":2}',0);
INSERT INTO task_events(task_id,kind_json,created_at)
VALUES('task-bound','{"kind":"follow_through_intent","intent":{"relation_id":"0b7c8f0e-1d2a-4e3b-9c4d-5e6f7a8b9c0d","issue_id":"task-new"}}',3),
      ('task-bound','{"kind":"follow_through_intent","intent":{"relation_id":"1b7c8f0e-1d2a-4e3b-9c4d-5e6f7a8b9c0d","issue_id":"task-git"}}',4),
      ('task-bound','{"kind":"follow_through_relation_confirmed","relation_id":"1b7c8f0e-1d2a-4e3b-9c4d-5e6f7a8b9c0d","source_issue_id":"linear-issue","target_issue_id":"x"}',5);
