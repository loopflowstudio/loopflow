
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
