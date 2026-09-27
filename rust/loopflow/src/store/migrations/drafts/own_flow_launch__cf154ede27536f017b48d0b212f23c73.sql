-- name: own_flow_launch
-- id: cf154ede27536f017b48d0b212f23c73
-- depends_on: point_task_at_invocation

-- The invocation row owns everything a saved Flow's driver needs to continue:
-- the launch facts join the captured graph, cursor, current attempt and
-- failure already here. Selectors resolve once at launch into task_id and
-- wave_id; no selector string is stored. A Task's own invocation keeps its
-- worktree on the Task, so its cwd stays NULL.
ALTER TABLE flow_invocations ADD COLUMN cwd TEXT;
ALTER TABLE flow_invocations ADD COLUMN message TEXT;
ALTER TABLE flow_invocations ADD COLUMN model TEXT;
ALTER TABLE flow_invocations ADD COLUMN wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT;
UPDATE flow_invocations SET wave_id=(
    SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id
    WHERE t.id=flow_invocations.task_id
) WHERE task_id IS NOT NULL;
-- A taskless saved Flow's Runs already carry the Wave it was launched for.
UPDATE flow_invocations SET wave_id=(
    SELECT min(wave_id) FROM runs WHERE invocation_id=flow_invocations.id
) WHERE task_id IS NULL AND wave_id IS NULL
    AND (SELECT count(DISTINCT wave_id) FROM runs
         WHERE invocation_id=flow_invocations.id AND wave_id IS NOT NULL)=1;
CREATE TRIGGER validate_invocation_wave_insert BEFORE INSERT ON flow_invocations
WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
    WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
)
BEGIN SELECT RAISE(ABORT, 'Invocation Task and Wave disagree'); END;
CREATE TRIGGER validate_invocation_wave_update BEFORE UPDATE OF task_id,wave_id ON flow_invocations
WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
    WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
)
BEGIN SELECT RAISE(ABORT, 'Invocation Task and Wave disagree'); END;
