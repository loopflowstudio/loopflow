-- A conversation can exist before any input is reserved. Admission alone does
-- not start backlog work; the first capture (or a bind of existing work) does.
DROP TRIGGER task_first_conversation_insert;
DROP TRIGGER task_first_conversation_bind;
CREATE TRIGGER task_first_conversation_bind AFTER UPDATE OF task_id ON agent_sessions
WHEN OLD.task_id IS NULL AND NEW.task_id IS NOT NULL AND NEW.current_capture IS NOT NULL BEGIN
    UPDATE tasks SET started_at=coalesce(NEW.bound_at,CAST(strftime('%s','now') AS INTEGER))
    WHERE id=NEW.task_id AND started_at IS NULL;
END;
CREATE TRIGGER task_first_input AFTER UPDATE OF current_capture ON agent_sessions
WHEN OLD.current_capture IS NULL AND NEW.current_capture IS NOT NULL AND NEW.task_id IS NOT NULL BEGIN
    UPDATE tasks SET started_at=(SELECT observed_at FROM session_events WHERE seq=NEW.current_capture)
    WHERE id=NEW.task_id AND started_at IS NULL;
END;
DROP TRIGGER validate_task_started_update;
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN NEW.started_at IS NOT NULL AND NOT (
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id AND current_capture IS NOT NULL) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_processes f JOIN processes driver ON driver.id=f.lf_process_id WHERE NEW.worktree!=''
            AND (driver.cwd=rtrim(NEW.worktree,'/') OR instr(driver.cwd,rtrim(NEW.worktree,'/')||'/')=1)) OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;
