-- Released Graph records: a finished run with a failed step, a live run with a
-- live step, and a usage-limit wait on that step.
INSERT INTO processes(id,trace_id,parent_lf_process_id,repo,cwd,started_at,completed_at,outcome,exit_code,error,pid,os_started_at)
VALUES('graph-done','trace',NULL,'/repo','/repo/task',10,30,'failed',1,'the run failed',NULL,NULL),
      ('step-done','trace','graph-done',NULL,'/repo/task',11,29,'failed',1,'gate: tests failed',NULL,NULL),
      ('graph-live','trace',NULL,'/repo','/repo/task',40,NULL,NULL,NULL,NULL,4001,39),
      ('step-live','trace','graph-live',NULL,'/repo/task',41,NULL,NULL,NULL,NULL,4002,40);
INSERT INTO graph_processes(lf_process_id,graph,compiled)
VALUES('graph-done','code','{}'),('graph-live','code','{}');
INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations)
VALUES('graph-done','step-done',0,'[]'),('graph-live','step-live',0,'[]');
INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,retry_at,agent,updated_at)
VALUES('step-live','claude','limit',100,NULL,41);
-- A Wave whose Project activation failed, and one whose activation still runs.
INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,outcome,exit_code,error,pid,os_started_at)
VALUES('activation-done','trace','/repo',20,25,'failed',1,'provider unavailable',NULL,NULL),
      ('activation-live','trace','/repo',42,NULL,NULL,NULL,NULL,4003,41),
      ('activation-next','trace','/repo',46,NULL,NULL,NULL,NULL,4004,45);
INSERT INTO waves(id,name,repo,created_at,project_activation_lf_process_id)
VALUES('wave-done','done','/repo',1,'activation-done'),('wave-live','live','/repo',1,'activation-live');
-- A settled CI repair, and a live one a released launcher still holds.
INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,outcome,exit_code,pid,os_started_at)
VALUES('repair-done','trace','/repo',60,70,'succeeded',0,NULL,NULL),
      ('repair-live','trace','/repo',80,NULL,NULL,NULL,5001,79),
      ('repair-worker','trace','/repo',90,NULL,NULL,NULL,5002,89);
INSERT INTO ci_incidents(identity,repo,pr_number,failed_head_sha,failure_set_json,poll_observed_at,created_at,updated_at,repair_lf_process_id,repair_finished_at)
VALUES('incident-done','/repo',1,'head','[]',1,1,1,'repair-done',71),
      ('incident-live','/repo',1,'head','[]',1,1,1,'repair-live',NULL);
-- A Task carried by a live released `lf task run`, which also ships it.
INSERT INTO processes(id,trace_id,cwd,started_at,pid,os_started_at)
VALUES('carrier-live','trace','/repo/task',100,6001,99),
      ('carrier-next','trace','/repo/task',110,6002,109);
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES('task-live','repo_1','RUN-1',1);
INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at)
VALUES('task-live','{}','start','{}','carrier-live',1);
INSERT INTO task_ship_events(task_id,kind,at,lf_process_id) VALUES('task-live','approved',100,'carrier-live');
-- A Session whose live agent a released launcher holds, and one whose agent exited.
INSERT INTO processes(id,trace_id,kind,started_at,completed_at,exit_code,pid,os_started_at,spawn_state)
VALUES('00000000-0000-4000-8000-00000000a001','00000000-0000-4000-8000-00000000a001','agent',120,NULL,NULL,7001,119,'spawn_requested'),
      ('00000000-0000-4000-8000-00000000a002','00000000-0000-4000-8000-00000000a002','agent',121,130,0,7002,120,'exited');
INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,agent_process_id)
VALUES('session-live','Live','human',1,0,'/repo','00000000-0000-4000-8000-00000000a001'),
      ('session-done','Done','human',1,0,'/repo','00000000-0000-4000-8000-00000000a002');
