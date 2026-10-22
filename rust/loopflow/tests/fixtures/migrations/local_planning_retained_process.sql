INSERT INTO processes(lfid,trace_id,command,cwd,started_at) VALUES('process-retained','trace','lf run code','/repo/task',2);
INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,driver_process_lfid,provider_thread,input_published) VALUES('session-retained','Conversation','human',2,'/repo/task','{task}','{wave}','process-retained','native-retained',1);
INSERT INTO task_workflows(task_id,graph,node,edge,process_lfid,updated_at) VALUES('{task}','{}','review',0,'process-retained',3);
INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,process_lfid,note,at) VALUES('{task}','code','chose','start','review',0,'process-retained','Original choice',3);
