-- Deleting a parent row, or changing its key, looks up every child that names
-- it. Without an index on the child column that lookup scans the child table
-- while holding the write lock.
--
-- A Process names its Session only when it is an agent; the old index also
-- filtered on kind, which a lookup by Session alone cannot use.
DROP INDEX agent_process_session;
CREATE INDEX processes_agent_session ON processes(agent_session_id)
    WHERE agent_session_id IS NOT NULL;
CREATE INDEX agent_sessions_wave ON agent_sessions(wave_id)
    WHERE wave_id IS NOT NULL;
CREATE INDEX steer_batches_agent_process ON steer_batches(agent_process_id);
CREATE INDEX steers_project ON steers(project_id)
    WHERE project_id IS NOT NULL;
CREATE INDEX tasks_checkout_machine ON tasks(checkout_machine_id);
CREATE INDEX tasks_parent_pr ON tasks(parent_pr_id)
    WHERE parent_pr_id IS NOT NULL;
