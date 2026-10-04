-- Completion and recovery checks ask each Task for its unfinished Execs.
-- Keep that read proportional to running work, not to retained history.
CREATE INDEX execs_unfinished ON execs(cwd, id) WHERE completed_at IS NULL;
