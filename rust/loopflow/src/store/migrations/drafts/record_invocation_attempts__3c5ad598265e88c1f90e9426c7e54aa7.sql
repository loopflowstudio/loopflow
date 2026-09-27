-- name: record_invocation_attempts
-- id: 3c5ad598265e88c1f90e9426c7e54aa7
-- depends_on: record_task_first_run
-- A position has many attempts. Historical rows without a captured location
-- stay unknown until the offline importer can establish their original tuple.
ALTER TABLE runs ADD COLUMN node INTEGER CHECK (node >= 0);
ALTER TABLE runs ADD COLUMN iterations TEXT CHECK (json_valid(iterations) AND json_type(iterations)='array');
ALTER TABLE runs ADD COLUMN attempt INTEGER CHECK (
    (node IS NULL AND iterations IS NULL AND attempt IS NULL) OR
    (invocation_id IS NOT NULL AND node IS NOT NULL AND iterations IS NOT NULL AND attempt IS NOT NULL AND attempt > 0)
);
CREATE INDEX run_position_attempts ON runs(invocation_id, node, iterations, attempt);
ALTER TABLE flow_invocations ADD COLUMN current_run_id TEXT REFERENCES runs(id);
UPDATE flow_invocations SET current_run_id=(
    SELECT current_run_id FROM sessions WHERE id=pending_session_id
);
CREATE TRIGGER validate_invocation_attempt BEFORE UPDATE OF current_run_id ON flow_invocations
WHEN NEW.current_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM runs WHERE id=NEW.current_run_id AND invocation_id=NEW.id
)
BEGIN SELECT RAISE(ABORT, 'Current attempt belongs to another Invocation'); END;
CREATE TRIGGER validate_initial_invocation_attempt BEFORE INSERT ON flow_invocations
WHEN NEW.current_run_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM runs WHERE id=NEW.current_run_id AND invocation_id=NEW.id
)
BEGIN SELECT RAISE(ABORT, 'Current attempt belongs to another Invocation'); END;
CREATE TRIGGER retain_run_position BEFORE UPDATE OF invocation_id,node,iterations,attempt ON runs
WHEN NEW.invocation_id IS NOT OLD.invocation_id OR NEW.node IS NOT OLD.node
    OR NEW.iterations IS NOT OLD.iterations OR NEW.attempt IS NOT OLD.attempt
BEGIN SELECT RAISE(ABORT, 'Run execution membership is immutable'); END;
