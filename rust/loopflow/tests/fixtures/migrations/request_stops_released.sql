INSERT INTO repository_plans(id,repo,selected)
VALUES('repo_00000000000040008000000000000001','/request-stop',1);
INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,created_at,ship_stopped_at)
VALUES('task_00000000000040008000000000000001','repo_00000000000040008000000000000001','STOP-1','Existing stop',1,25);
