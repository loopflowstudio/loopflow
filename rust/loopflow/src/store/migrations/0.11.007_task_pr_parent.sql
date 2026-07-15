-- A dependent Task keeps its own worktree and PR, placed on another Task's PR.
-- The fork commit makes the eventual collapse onto main squash-safe; the id is
-- durable ownership evidence when readable branch names change or disappear.
ALTER TABLE task_prs ADD COLUMN parent_pr_id TEXT REFERENCES task_prs(id);
