-- draft: task_pr_continuation
-- Retain one current PR while allowing continued work in the same Task.
DROP TRIGGER task_pr_history_no_mark;
CREATE TRIGGER task_pr_history_no_mark BEFORE UPDATE OF historical ON task_prs
WHEN NEW.historical<>OLD.historical AND NOT (
 OLD.historical=0 AND NEW.historical=1
 AND (OLD.merge_commit IS NOT NULL OR OLD.abandoned_at IS NOT NULL))
BEGIN SELECT RAISE(ABORT, 'only settled Task PRs can become history'); END;

-- Settled PRs describe the delivered checkout, not its later working base.
DROP TRIGGER task_placement_to_pr;
CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;
