-- Attachment transfer is a new claim, including A -> B -> A. No counter is
-- executable authority after migration. Existing event keys/payloads stay intact.
ALTER TABLE agent_sessions ADD COLUMN attachment_token TEXT;
ALTER TABLE agent_sessions ADD COLUMN attachment_exit_seq INTEGER REFERENCES session_events(seq);
UPDATE agent_sessions SET attachment_token = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))),2) || '-8' ||
    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))
) WHERE driver_generation > 0;
UPDATE agent_sessions SET attachment_exit_seq = (
    SELECT seq FROM session_events e WHERE e.session_id=agent_sessions.id
      AND e.kind='observed' AND e.receipt_key='driver:'||(agent_sessions.driver_generation-1)||':exit'
);
ALTER TABLE session_activity ADD COLUMN attachment_token TEXT;
UPDATE session_activity SET attachment_token = (
    SELECT s.attachment_token FROM agent_sessions s WHERE s.id=session_activity.session_id
      AND s.driver_generation=session_activity.driver_generation
);
DROP TRIGGER store_revision_session_activity_update;
ALTER TABLE session_activity DROP COLUMN driver_generation;
ALTER TABLE agent_sessions DROP COLUMN driver_generation;
ALTER TABLE agent_sessions RENAME COLUMN driver_process_lfid TO attached_process_lfid;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.attachment_token IS NOT OLD.attachment_token
    OR NEW.provider_generation IS NOT OLD.provider_generation
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
