-- Released 0.13.11 fields; journal seeding must retain these values and receipts.
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-proof','proof','/repo',1);
INSERT INTO projects(id,wave_id,created_at,planning_teams,planning_initiatives)
VALUES('project-proof','wave-proof',1,'["team-a","team-b"]','["initiative-a"]');
INSERT INTO tasks(id,project_id,issue_identifier,created_at,updated_at)
VALUES('task-proof','project-proof','PROOF-1',1,1);
INSERT INTO task_comments(id,task_id,body,author,created_at,provider_revision)
VALUES('comment-proof','task-proof','Keep attribution','{"kind":"person","name":"Maya"}',NULL,'revision-before');
INSERT INTO task_comment_deliveries(comment_id,comment_json,error)
VALUES('comment-proof','{"id":"comment-proof","body":"Keep attribution","author":{"kind":"person","name":"Maya"},"created_at":null}','Lost response');
