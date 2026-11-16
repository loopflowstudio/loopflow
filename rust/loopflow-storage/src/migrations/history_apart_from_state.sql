-- A viewer heartbeat is a reading time, not a change. The holder re-stamps
-- `viewer_hold_at` every ten seconds per viewed terminal; counting each stamp
-- woke every subscription about five times a second. Retirement reads the
-- stamp on its own clock, so only its first value counts, beside every other
-- column.
DROP TRIGGER store_revision_held_terminals_update;
CREATE TRIGGER store_revision_held_terminals_update AFTER UPDATE ON held_terminals
WHEN NEW.id IS NOT OLD.id OR NEW.holder_build IS NOT OLD.holder_build OR
    NEW.created_at IS NOT OLD.created_at OR NEW.ended_at IS NOT OLD.ended_at OR
    NEW.exit_code IS NOT OLD.exit_code OR NEW.viewer_hold IS NOT OLD.viewer_hold OR
    NEW.viewed IS NOT OLD.viewed OR
    (NEW.viewer_hold_at IS NULL) IS NOT (OLD.viewer_hold_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;

-- Nothing writes or reads `task_update` observations since their owner was
-- deleted; only Graph Session facts still move Task updates.
DROP TRIGGER task_update_session_events_insert;
CREATE TRIGGER task_update_session_events_insert AFTER INSERT ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

DROP TRIGGER task_update_session_events_update;
CREATE TRIGGER task_update_session_events_update AFTER UPDATE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

DROP TRIGGER task_update_session_events_delete;
CREATE TRIGGER task_update_session_events_delete AFTER DELETE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

-- Which conversation `lf resume` reopens is state: the latest time a person
-- opened it, in milliseconds. It was an `interactive_opened` observation per
-- opening; the history file keeps no copy.
ALTER TABLE agent_sessions ADD COLUMN opened_at INTEGER;
UPDATE agent_sessions SET opened_at=(
    SELECT MAX(json_extract(payload,'$.opened_at_ms')) FROM session_events
    WHERE session_id=agent_sessions.id AND kind='observed'
      AND json_valid(payload) AND json_extract(payload,'$.type')='interactive_opened')
WHERE interactive=1;
