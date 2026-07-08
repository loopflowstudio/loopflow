-- The prompt-unit noun is skill; step is retired. Rename every live column
-- (journal and metrics files are per-machine, not migrated).
ALTER TABLE terminal_sessions RENAME COLUMN step TO skill;
ALTER TABLE run_events RENAME COLUMN step TO skill;
ALTER TABLE run_events RENAME COLUMN step_index TO skill_index;
ALTER TABLE runs RENAME COLUMN step_index TO skill_index;
ALTER TABLE fork_runs RENAME COLUMN step_index TO skill_index;
