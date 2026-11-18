INSERT INTO processes(lfid,trace_id,command,cwd,started_at)
    VALUES('held','trace','lf run proof','/single',2);
INSERT INTO task_workflows(task_id,graph,node,edge,process_lfid,updated_at)
    VALUES('single','{"name":"proof","nodes":[],"edges":[]}','start',0,'held',2);
