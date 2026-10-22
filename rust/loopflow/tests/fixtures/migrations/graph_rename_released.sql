-- Released Flow records: a Flow process with one step, a Task mid-edge
-- on a Workflow whose edges name `flow`, and a Wave workflow authored with `flow:`.
INSERT INTO processes(id,trace_id,command,cwd,repo,started_at)
    VALUES('00000000-0000-4000-8000-00000000f100','00000000-0000-4000-8000-0000000000aa','["lf","flow","pursue"]','{repo}','{repo}',1);
INSERT INTO processes(id,trace_id,parent_lf_process_id,command,cwd,repo,started_at,completed_at,outcome,exit_code)
    VALUES('00000000-0000-4000-8000-00000000f101','00000000-0000-4000-8000-0000000000aa','00000000-0000-4000-8000-00000000f100','["lf","skill","implement"]','{repo}','{repo}',2,3,'succeeded',0);
INSERT INTO flow_processes(lf_process_id,flow,graph) VALUES('00000000-0000-4000-8000-00000000f100','pursue','{compiled}');
INSERT INTO flow_process_steps(flow_lf_process_id,lf_process_id,node,iterations)
    VALUES('00000000-0000-4000-8000-00000000f100','00000000-0000-4000-8000-00000000f101',0,'[]');
INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','{repo}',1);
INSERT INTO waves(id,name,repo,created_at) VALUES('{wave}','product','{repo}',1);
INSERT INTO projects(id,wave_id,created_at) VALUES('{project}','{wave}',1);
INSERT INTO tasks(id,repository_id,project_id,issue_identifier,created_at)
    VALUES('{task}','repo_00000000000040008000000000000001','{project}','PROOF-1',1);
INSERT INTO task_workflows(task_id,graph,node,edge,lf_process_id,updated_at)
    VALUES('{task}','{"name":"feature","nodes":[{"name":"demo","skill":"demo","description":null}],"edges":[{"from":"start","to":"demo","flow":"pursue"},{"from":"demo","to":"end","flow":null}]}','start',0,'00000000-0000-4000-8000-00000000f100',2);
INSERT INTO wave_workflows(wave_id,name,content)
    VALUES('{wave}','feature','# Build, then try it.
nodes:
  demo: { skill: demo, description: you try the change }
edges:
  - { from: start, to: demo, flow: pursue }
  - from: demo
    to: end
    flow: ship
');
