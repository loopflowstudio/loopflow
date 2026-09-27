-- name: name_tasks_on_flow_step_runs
-- id: 24f1227c06793e7da8da1d1c286089b6
-- depends_on: record_run_end

-- A Flow launched directly for a Task owns its own invocation, which names no
-- Task: a Task's invocation is its managed Flow. The Flow's step Runs still
-- name the Task they work for. An invocation that names a Task fills it on
-- its Runs, and a different Task is refused.
DROP TRIGGER validate_run_parents_insert;
DROP TRIGGER validate_run_parents_update;
CREATE TRIGGER validate_run_parents_insert BEFORE INSERT ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id
        AND (f.task_id IS NULL OR f.task_id IS NEW.task_id)
    ) THEN RAISE(ABORT, 'Run and Invocation Tasks disagree') END;
END;
CREATE TRIGGER validate_run_parents_update BEFORE UPDATE OF task_id,wave_id,invocation_id ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id
        AND (f.task_id IS NULL OR f.task_id IS NEW.task_id)
    ) THEN RAISE(ABORT, 'Run and Invocation Tasks disagree') END;
END;
