-- Persist the parent PR and fork commit used to place dependent Task work.
-- Branch names remain readable hints; these columns are the durable truth.
ALTER TABLE task_prs ADD COLUMN parent_pr_id TEXT REFERENCES task_prs(id);

-- Separate Tasks can be active concurrently while the child waits on its
-- parent's merge. Each Task still owns at most one open PR at a time.
