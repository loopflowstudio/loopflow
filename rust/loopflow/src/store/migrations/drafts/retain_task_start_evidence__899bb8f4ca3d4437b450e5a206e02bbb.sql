-- name: retain_task_start_evidence
-- id: 899bb8f4ca3d4437b450e5a206e02bbb
-- depends_on: retain_imported_session_evidence

-- Released Tasks can have Started evidence without a mapped conversation.
-- As with record_task_first_run, conversion establishes assignment presence;
-- the original event retains its own chronology. Never replace an existing
-- assignment timestamp with the historical event or a new conversion time.
DROP TRIGGER validate_task_started_update;
UPDATE tasks SET started_at=CAST(strftime('%s','now') AS INTEGER)
WHERE started_at IS NULL AND EXISTS (
    SELECT 1 FROM task_events e WHERE e.task_id=tasks.id
    AND json_extract(e.kind_json,'$.kind')='started'
);
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM runs WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started') OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;
