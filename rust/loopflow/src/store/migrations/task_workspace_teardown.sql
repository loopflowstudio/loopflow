-- Automatic shell closes are decided in each cleanup pass, never queued.
-- Explicit Close stays on its released receipt and survives an upgrade.
UPDATE desktop_shells SET close_requested=0 WHERE close_requested=2;
UPDATE desktop_shells SET jobs_settled=1 WHERE owner IS NULL AND close_requested=1;

-- The existing checkout retry queue also owns abandonment cleanup. Reopening or
-- changing its placement discards that intent, just as for completed Tasks.
DROP TRIGGER cancel_task_retirement;
CREATE TRIGGER cancel_task_retirement
AFTER UPDATE OF planning_completed,planning_completed_at,abandoned_at,worktree,branch ON tasks
WHEN (NEW.planning_completed=0 AND NEW.abandoned_at IS NULL)
  OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.abandoned_at IS NOT OLD.abandoned_at
  OR NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN
    DELETE FROM task_retirements WHERE task_id=NEW.id;
END;
