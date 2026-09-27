-- name: point_task_at_invocation
-- id: ef3fa1278026a9a2f7bdf0f19fd32209
-- depends_on: name_tasks_on_flow_step_runs

-- A Task points at the one invocation that advances it. Every other Flow
-- launched for the Task names it on its invocation and its Runs, reviews
-- included, without becoming the Task's Flow. A Run and its invocation agree
-- exactly on their nullable Task again.
ALTER TABLE tasks ADD COLUMN current_invocation_id TEXT REFERENCES flow_invocations(id);
UPDATE tasks SET current_invocation_id=(
    SELECT id FROM flow_invocations WHERE task_id=tasks.id AND state='current'
);
DROP INDEX task_current_invocation;
CREATE TRIGGER validate_task_invocation BEFORE UPDATE OF current_invocation_id ON tasks
WHEN NEW.current_invocation_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM flow_invocations WHERE id=NEW.current_invocation_id AND task_id=NEW.id
)
BEGIN SELECT RAISE(ABORT, 'Task invocation names another Task'); END;

DROP TRIGGER validate_run_parents_insert;
DROP TRIGGER validate_run_parents_update;
CREATE TRIGGER validate_run_parents_insert BEFORE INSERT ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id AND f.task_id IS NEW.task_id
    ) THEN RAISE(ABORT, 'Run and Invocation nullable Tasks disagree') END;
END;
CREATE TRIGGER validate_run_parents_update BEFORE UPDATE OF task_id,wave_id,invocation_id ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id AND f.task_id IS NEW.task_id
    ) THEN RAISE(ABORT, 'Run and Invocation nullable Tasks disagree') END;
END;
