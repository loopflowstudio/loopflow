INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,workspace_slug,agent)
VALUES('retained','repo_00000000000040008000000000000001','TEST-1',1,'/repo.task','retained','claude');
INSERT INTO agent_sessions(id,title,title_source,interactive,created_at,repo,task_id,cwd,input_published,provider)
VALUES('history','Retained conversation','human',1,1,'/repo','retained','/repo.task',1,'claude');
INSERT INTO scope_settings(scope_kind,scope_id,key,value,set_at)
VALUES('global','','skill_agents.implement','codex',1),('global','','skill_agents.design','claude',1),
      ('global','','skills.design.agent','codex:o3',2);
