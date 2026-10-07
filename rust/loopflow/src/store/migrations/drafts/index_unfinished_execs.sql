-- Completion and recovery checks ask each Task for its unfinished Execs.
-- Keep that read proportional to running work, not to retained history.
CREATE INDEX execs_unfinished ON execs(cwd, id) WHERE completed_at IS NULL;

-- Task and Flow membership need only the events that name an Exec: a few
-- thousand entries, not a table seek for every retained event.
CREATE INDEX session_exec_membership ON session_events(session_id,exec_id) WHERE exec_id IS NOT NULL;
