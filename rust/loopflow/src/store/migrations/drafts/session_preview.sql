ALTER TABLE session_activity ADD COLUMN preview TEXT CHECK (preview IS NULL OR json_valid(preview));
DROP TRIGGER store_revision_session_activity_update;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.attachment_token IS NOT OLD.attachment_token
    OR NEW.agent_process_id IS NOT OLD.agent_process_id
    OR NEW.preview IS NOT OLD.preview
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;

CREATE TRIGGER session_preview_agent_session_update
AFTER UPDATE OF provider_thread,agent_process_id ON agent_sessions
WHEN NEW.provider_thread IS NOT OLD.provider_thread OR NEW.agent_process_id IS NOT OLD.agent_process_id
BEGIN
    UPDATE session_activity SET preview=NULL WHERE session_id=NEW.id;
END;
