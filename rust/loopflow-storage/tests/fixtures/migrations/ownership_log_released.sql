-- Released 0.14.8 Started, checkout, and admission evidence before the ownership log.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_owned','/repo',1);
INSERT INTO machines(id,route,created_at,observed_at)
VALUES('home_checkout','ssh://checkout',1,1),('home_admitted','ssh://admitted',1,1);
INSERT INTO machines(id,route,created_at,observed_at,label,repo)
VALUES('home_labeled','ssh://mini',1,1,'mini','src/project');
INSERT INTO tasks(id,repository_id,issue_identifier,created_at,workspace_slug,worktree,checkout_machine_id)
VALUES('task_started','repo_owned','OWN-1',10,'',NULL,NULL),
      ('task_checkout','repo_owned','OWN-2',11,'','/repo.checkout','home_checkout'),
      ('task_admitted','repo_owned','OWN-3',12,'',NULL,NULL),
      ('task_unowned','repo_owned','OWN-4',13,'',NULL,NULL);
INSERT INTO task_events(task_id,kind_json,created_at) VALUES('task_started','{"kind":"started"}',20);
UPDATE tasks SET started_at=20 WHERE id='task_started';
INSERT INTO task_admissions(task_id,machine_id) VALUES('task_admitted','home_admitted');
INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES
    ('retired-1','tasks','{"edit":{"target":"task_checkout","change":{"field":"task_delegation","value":null}},"clock":1,"parents":[]}'),
    ('kept-1','tasks','{"edit":{"target":"task_checkout","change":{"field":"task_title","value":"Kept"}},"clock":2,"parents":[]}');
INSERT INTO planning_peer_observed(id) VALUES('retired-1'),('kept-1');
