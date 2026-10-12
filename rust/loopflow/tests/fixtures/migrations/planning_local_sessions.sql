-- Released 0.13.11 execution evidence. Planning conversion cannot settle or rebind it.
INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,provider,provider_thread,input_published)
VALUES('session-proof','Retained conversation','human',2,'/retained/task','task-proof','wave-proof','codex','native-proof',1),
      ('session-reserved','Reserved input','generated',3,'/retained/task','task-proof','wave-proof','claude','native-reserved',0);
INSERT INTO processes(id,trace_id,command,cwd,started_at,kind,agent_session_id,pid,os_started_at,endpoint,
                      agent_provider,agent_interactive,parent_lf_process_id,attached_lf_process_id,attachment_token,spawn_state)
VALUES('22222222-2222-4222-8222-222222222222','trace','codex','/retained/task',3,'agent','session-proof',2147483000,3,
       '/retained/provider.sock','codex',1,'11111111-1111-4111-8111-111111111111','11111111-1111-4111-8111-111111111111',
       '33333333-3333-4333-8333-333333333333','spawn_requested');
UPDATE agent_sessions SET agent_process_id='22222222-2222-4222-8222-222222222222' WHERE id='session-proof';
UPDATE tasks SET primary_session_id='session-proof' WHERE id='task-proof';
INSERT INTO session_events(seq,session_id,provider_thread,provider_turn,kind,receipt_key,lf_process_id,task_id,wave_id,observed_at,payload,captured_event,agent_process_id)
VALUES(100,'session-proof','native-proof',NULL,'captured','retained-input','11111111-1111-4111-8111-111111111111','task-proof','wave-proof',3,
       '{"cwd":"/retained/task","skill":"implement","request":"Keep working"}',NULL,NULL),
      (101,'session-proof','native-proof','earlier','started','native:earlier:started','11111111-1111-4111-8111-111111111111','task-proof','wave-proof',4,
       '{}',100,'22222222-2222-4222-8222-222222222222'),
      (102,'session-proof','native-proof','earlier','completed','native:earlier:completed','11111111-1111-4111-8111-111111111111','task-proof','wave-proof',5,
       '{"status":"completed"}',100,'22222222-2222-4222-8222-222222222222'),
      (103,'session-proof','native-proof','current','started','native:current:started','11111111-1111-4111-8111-111111111111','task-proof','wave-proof',6,
       '{}',100,'22222222-2222-4222-8222-222222222222'),
      (104,'session-reserved','native-reserved',NULL,'captured','reserved-input',NULL,'task-proof','wave-proof',7,
       '{"cwd":"/retained/task","skill":"review","request":"Keep this reservation"}',NULL,NULL);
UPDATE agent_sessions SET current_capture=100 WHERE id='session-proof';
UPDATE agent_sessions SET current_capture=104 WHERE id='session-reserved';
INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded,attachment_token,agent_process_id,program_status,status_stream,status_sequence)
VALUES('session-proof',9,2,1,0,'33333333-3333-4333-8333-333333333333','22222222-2222-4222-8222-222222222222',
       '{"state":"working","kind":"task","message":"Preserve provider control"}','retained-stream',8);
INSERT INTO task_events(task_id,kind_json,created_at)
VALUES('task-proof','{"kind":"interrupt"}',9);
INSERT INTO task_state_deliveries(id,task_id,target,attempted,error)
VALUES('retained-state','task-proof','canceled',1,'Lost local response');
