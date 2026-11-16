PRAGMA foreign_keys = ON;
INSERT INTO execs(id,trace_id,command,cwd,started_at)
    VALUES('parent','trace','lf run code','/repo/task',1);
INSERT INTO execs(id,trace_id,parent_exec_id,command,started_at,completed_at,outcome,exit_code,error)
    VALUES('child','trace','parent','lf skill implement',2,3,'failed',42,'retained error');
INSERT INTO waves(id,name,repo,created_at,project_activation_exec_id) VALUES('w','proof','/repo',1,'parent');
INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('p','w','external-p',1);
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree)
    VALUES('t','p','external-t','PROOF-1',1,'/repo/task');
INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,driver_exec_id,provider_exec_id,provider_thread,input_published)
    VALUES('s','Keep conversation','human',1,'/repo/task','t','w','child','child','native-thread',1);
INSERT INTO session_events(session_id,exec_id,kind,receipt_key,observed_at,payload)
    VALUES('s','child','captured','capture',2,'{"exec":"opaque history"}');
INSERT INTO flow_execs(exec_id,flow,graph) VALUES('parent','code','{}');
INSERT INTO flow_exec_steps(flow_exec_id,exec_id,node,iterations) VALUES('parent','child',7,'[[2,1]]');
INSERT INTO task_workflows(task_id,graph,node,edge,exec_id,updated_at) VALUES('t','{}','start',0,'parent',2);
INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,exec_id,note,at)
    VALUES('t','code','chose','start','start',0,'parent','Retained decision',2);
