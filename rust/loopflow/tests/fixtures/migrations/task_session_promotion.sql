-- Released shared primary and ordinary Wave conversations, with immutable input.
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_promotion','/repo',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('wave-promotion','research','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,created_at,updated_at)
VALUES('task_promotion','repo_promotion','PROMOTE-1','Continue research',1,1),
      ('task_other','repo_promotion','PROMOTE-2','Other work',1,1);
INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,repo,wave_id,primary_scope,interactive,input_published)
VALUES('primary','Research','human',1,'/repo/wave','/repo','wave-promotion','wave',1,1),
      ('ordinary','Contribution','human',1,'/repo/wave','/repo','wave-promotion',NULL,1,0);
INSERT INTO session_events(session_id,kind,receipt_key,wave_id,observed_at,payload)
VALUES('primary','captured','retained-input','wave-promotion',1,'{}');
