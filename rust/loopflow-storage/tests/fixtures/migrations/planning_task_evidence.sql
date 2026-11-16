-- Released 0.13.11: owned Work plus partial and foreign Linear observations.
INSERT INTO waves(id,name,repo,created_at)
VALUES('{wave}','product','/repo',1);
INSERT INTO projects(id,wave_id,external_project_id,created_at,project_name,project_slug,project_prompt_context,planning_teams,updated_at)
VALUES('{project}','{wave}','project',1,'Saved Project','saved','Preserved prose','["team"]',7);
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,issue_title,issue_description,worktree,workspace_slug,updated_at)
VALUES('{task}','{project}','issue','TEAM-1',1,'Saved Task','Saved brief','/retained/checkout','retained',7);

-- Content is deliberately incomplete: reading membership must not reconstruct it.
INSERT INTO pm_projects(repo,provider,id,observed_at,body)
VALUES('/repo','linear','project',31,'{"team_ids":["team"],"initiative_ids":["initiative"]}'),
      ('/repo','linear','unmapped',29,'{"team_ids":["foreign"]}');
INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body)
VALUES('/repo','linear','issue','TEAM-1','project',42,'{"team_id":"team","project_id":"project"}'),
      ('/repo','linear','foreign','OTHER-2','unmapped',19,'{"team_id":"foreign"}');
