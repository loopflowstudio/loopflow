-- Cleanup resolves retained branches without scanning unrelated Task/PR history.
CREATE INDEX tasks_branch ON tasks(branch);
CREATE INDEX task_prs_branch ON task_prs(branch);
