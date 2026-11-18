PRAGMA foreign_keys=ON;
            INSERT INTO processes(lfid,trace_id,started_at) VALUES('parent','trace',1);
            INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,input_published,driver_process_lfid,driver_generation,provider_generation,provider_process_lfid,provider_thread,provider_pid,provider_started_at)
              VALUES('live','Retained','human',1,'/repo',1,'parent',7,3,'parent','native',42,100),
                    ('released','Retained exit','human',1,'/repo',1,NULL,8,3,'parent','history',NULL,NULL),
                    ('stale','Old reading','human',1,'/repo',1,'parent',7,3,'parent','native-stale',NULL,NULL);
            INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
              VALUES('released','observed','driver:7:exit',2,'{"type":"driver_exit","outcome":"interrupted"}'),
                    ('live','captured','input',1,'{"text":"keep pending input"}');
            UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events WHERE receipt_key='input') WHERE id='live';
            INSERT INTO session_activity(session_id,driver_generation,observed_at,open_tools,pending_input,yielded,provider_generation)
              VALUES('live',7,100,0,1,0,3),('stale',6,90,0,1,0,3);
            INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,provider_generation,process_lfid,observed_at,payload)
              VALUES('live','native','current','started','native:current:started',3,'parent',3,'{}'),
                    ('live','native','earlier','started','native:earlier:started',2,'parent',2,'{}');
            INSERT INTO processes(lfid,trace_id,started_at,via_agent,caller_session_id,caller_provider_generation)
              VALUES('current-child','trace',4,1,'live',3),('earlier-child','trace',3,1,'live',2);
