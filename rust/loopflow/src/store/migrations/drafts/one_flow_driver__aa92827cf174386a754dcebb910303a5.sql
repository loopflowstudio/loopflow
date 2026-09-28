-- name: one_flow_driver
-- id: aa92827cf174386a754dcebb910303a5
-- depends_on: own_flow_launch

-- One driver reads every invocation through one SELECT. A Task invocation
-- runs in the Task worktree, so its row stores no cwd of its own; the read
-- coalesces the Task's worktree. A Task is started by its first Run, the
-- worker claim's unpublished reservation included, so the Started event
-- writer goes. Claims written by the previous driver named a Run the row now
-- owns as its current attempt. Keep the claim and its exact process fence;
-- only the redundant Run field leaves its serialization.
DROP TRIGGER task_chapter_started;
UPDATE flow_invocations SET cwd=NULL WHERE task_id IS NOT NULL;
CREATE TRIGGER validate_invocation_cwd_insert BEFORE INSERT ON flow_invocations
WHEN NEW.task_id IS NOT NULL AND NEW.cwd IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'A Task invocation runs in the Task worktree'); END;
CREATE TRIGGER validate_invocation_cwd_update BEFORE UPDATE OF task_id,cwd ON flow_invocations
WHEN NEW.task_id IS NOT NULL AND NEW.cwd IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'A Task invocation runs in the Task worktree'); END;
UPDATE flow_invocations SET claim_json=json_remove(claim_json, '$.worker_run_id')
WHERE claim_json IS NOT NULL;
