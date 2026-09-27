-- name: record_task_first_run
-- id: b00a8bb58c57bf79085aba8e56c3f43b
-- depends_on: record_run_work_source

ALTER TABLE tasks ADD COLUMN started_at INTEGER;

-- Existing rows establish presence, not the time their Task was first assigned.
-- Use conversion time for this historical inference. The offline Home import
-- report must distinguish it from subsequently observed assignment times.
UPDATE tasks SET started_at=CAST(strftime('%s','now') AS INTEGER)
WHERE EXISTS (SELECT 1 FROM runs WHERE task_id=tasks.id);

-- These triggers are the common assignment writer for constructor, bind and
-- offline import. Reservation counts; publication and provider outcome do not.
CREATE TRIGGER task_first_run_insert AFTER INSERT ON runs
WHEN NEW.task_id IS NOT NULL
BEGIN
    UPDATE tasks SET started_at=CAST(strftime('%s','now') AS INTEGER)
    WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER task_first_run_bind AFTER UPDATE OF task_id ON runs
WHEN OLD.task_id IS NULL AND NEW.task_id IS NOT NULL
BEGIN
    UPDATE tasks SET started_at=CAST(strftime('%s','now') AS INTEGER)
    WHERE id=NEW.task_id AND started_at IS NULL;
END;

CREATE TRIGGER validate_task_started_insert BEFORE INSERT ON tasks
WHEN NEW.started_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Started requires a Task Run'); END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks
BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT, 'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != EXISTS (
        SELECT 1 FROM runs WHERE task_id=NEW.id
    ) THEN RAISE(ABORT, 'Started requires a Task Run') END;
END;

-- Task attribution is permanent. Wave-only binding fills a missing Task but
-- cannot move its Wave; Task ancestry changes remain a separate transaction.
CREATE TRIGGER validate_run_assignment BEFORE UPDATE OF task_id,wave_id ON runs
BEGIN
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT, 'Run Task assignment cannot change or clear') END;
    SELECT CASE WHEN OLD.task_id IS NULL AND OLD.wave_id IS NOT NULL
        AND NEW.wave_id IS NOT OLD.wave_id
        THEN RAISE(ABORT, 'Bind cannot change or clear the existing Wave') END;
END;

-- Retained history is the evidence for the monotonic Started timestamp.
CREATE TRIGGER retain_task_first_run BEFORE DELETE ON runs
WHEN OLD.task_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM runs WHERE task_id=OLD.task_id AND id!=OLD.id
)
BEGIN SELECT RAISE(ABORT, 'Cannot remove the last Run of a started Task'); END;
