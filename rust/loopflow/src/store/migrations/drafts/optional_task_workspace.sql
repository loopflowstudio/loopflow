-- A Flow owns its launch directory independently of optional Task delivery.
DROP TRIGGER validate_invocation_cwd_insert;
DROP TRIGGER validate_invocation_cwd_update;

-- Freeze the directory previously projected from retained Task placement.
-- Missing historical placement stays unknown; do not invent the caller's cwd.
UPDATE flow_sessions
SET cwd = (SELECT worktree FROM tasks WHERE id = flow_sessions.task_id)
WHERE cwd IS NULL AND task_id IS NOT NULL;
