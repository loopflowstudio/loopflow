-- Released Graphs, paired with Tasks only by the read-time rule.
INSERT INTO machines(id,created_at,observed_at,is_local)
VALUES('HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,2,1);
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_graphs','/repo',1);
INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,worktree,checkout_machine_id,created_at,updated_at)
VALUES('task_outer','repo_graphs','GRAPH-1','Outer','/repo','HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,1),
      ('task_inner','repo_graphs','GRAPH-2','Inner','/repo/task/','HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,1),
      ('task_sibling','repo_graphs','GRAPH-3','Sibling','/repo/task-other','HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,1),
      ('task_bound','repo_graphs','GRAPH-4','Bound','/bound','HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',1,1);
INSERT INTO processes(id,trace_id,cwd,started_at)
VALUES('graph-deep','trace','/repo/task/deep/er',1),
      ('graph-sibling','trace','/repo/task-other/x',1),
      ('graph-moved','trace','/repo/task',1),
      ('graph-session','trace','/elsewhere',1),
      ('graph-outside','trace','/nowhere',1);
INSERT INTO graph_processes(lf_process_id,graph,compiled)
VALUES('graph-deep','code','{}'),('graph-sibling','code','{}'),('graph-moved','code','{}'),
      ('graph-session','code','{}'),('graph-outside','code','{}');
INSERT INTO relocated_task_processes(task_id,process_id) VALUES('task_bound','graph-moved');
INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,interactive,input_published)
VALUES('bound','Bound','human',1,'/elsewhere','task_bound',1,0);
INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
VALUES('bound','started','graph-session','graph-session',1,'{}');
