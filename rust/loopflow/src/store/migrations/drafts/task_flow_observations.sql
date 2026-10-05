
-- CI repair retains its existing per-Task hold. Saved Flow scheduling is retired.
ALTER TABLE tasks DROP COLUMN automation_exec_id;
ALTER TABLE tasks DROP COLUMN automation_retry_key;
ALTER TABLE tasks DROP COLUMN automation_retries;
ALTER TABLE tasks DROP COLUMN automation_checked_at;
ALTER TABLE tasks DROP COLUMN automation_detail;

-- Feedback remains Session history; it no longer grants readiness or settlement.
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT id,'observed','legacy_review_feedback',task_id,wave_id,unixepoch(),
    json_object('type','legacy_review_feedback','summary',ready_summary),current_capture
FROM agent_sessions WHERE ready_summary IS NOT NULL;
ALTER TABLE agent_sessions DROP COLUMN ready_summary;

-- Keep unresolved review boundaries as observations on their original conversation.
-- Neither conversion nor later native resume approves or advances the saved Flow.
INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
SELECT s.id,'observed','legacy_flow_review',s.task_id,s.wave_id,unixepoch(),
    json_object('type','legacy_flow_review','flow_id',s.flow_session_id,
        'pending',s.completed_at IS NULL AND EXISTS(SELECT 1 FROM flow_sessions f WHERE f.pending_session_id=s.id AND f.state='current'),
        'completed_at',s.completed_at),s.current_capture
FROM agent_sessions s WHERE s.kind='flow_review';
UPDATE agent_sessions SET kind='conversation' WHERE kind='flow_review';
