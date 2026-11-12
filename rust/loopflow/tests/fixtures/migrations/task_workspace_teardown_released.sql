INSERT INTO repository_plans(id,repo,selected) VALUES('repo_closes','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
VALUES('task_closes','repo_closes','CLOSE-1',1);
INSERT INTO processes(id,trace_id,command,cwd,started_at)
VALUES('explicit_owner','explicit_owner','["lf"]','/repo',1),
      ('automatic_owner','automatic_owner','["lf"]','/repo',1);
INSERT INTO desktop_shells(id,task_id,checkout,owner,created_at,close_requested)
VALUES('explicit','task_closes','/repo','explicit_owner',1,1),
      ('automatic','task_closes','/repo','automatic_owner',1,2),
      ('unstarted','task_closes','/repo',NULL,1,1);
