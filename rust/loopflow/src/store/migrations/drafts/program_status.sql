
ALTER TABLE session_activity ADD COLUMN provider_generation INTEGER;
ALTER TABLE session_activity ADD COLUMN program_status TEXT CHECK (program_status IS NULL OR json_valid(program_status));
ALTER TABLE session_activity ADD COLUMN status_stream TEXT;
ALTER TABLE session_activity ADD COLUMN status_sequence INTEGER;
-- Prior inference has no provider-generation witness; wait for a fresh reading.
DROP TRIGGER store_revision_session_activity_update;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.driver_generation IS NOT OLD.driver_generation
    OR NEW.provider_generation IS NOT OLD.provider_generation
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
