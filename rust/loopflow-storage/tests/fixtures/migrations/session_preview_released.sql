INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,input_published,interactive)
VALUES('retained','Retained conversation','human',1,'/repo',0,1);
INSERT INTO session_activity(session_id,observed_at,open_tools,pending_input,yielded)
VALUES('retained',100,0,1,0);
