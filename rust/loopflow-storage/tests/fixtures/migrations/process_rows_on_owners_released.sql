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
