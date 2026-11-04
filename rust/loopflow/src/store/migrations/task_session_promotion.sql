-- Promotion may release a shared primary scope before binding the conversation
-- to a Task. Permanent Task binds and ordinary Wave assignments stay intact.
DROP TRIGGER validate_conversation_ancestry_update;
CREATE TRIGGER validate_conversation_ancestry_update BEFORE UPDATE OF task_id,wave_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'Session Task binding is permanent') END;
    SELECT CASE WHEN OLD.task_id IS NULL AND OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        AND NOT (OLD.primary_scope IS 'wave' AND NEW.primary_scope IS NULL AND NEW.wave_id IS NULL)
        THEN RAISE(ABORT,'Wave Session binding is permanent') END;
END;

-- A requested workspace is applied only at the next idle attachment boundary.
-- cwd continues to describe the current launch until then.
ALTER TABLE agent_sessions ADD COLUMN requested_cwd TEXT;
