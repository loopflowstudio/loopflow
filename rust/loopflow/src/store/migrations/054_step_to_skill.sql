-- The prompt-unit noun is skill (a step is the generic flow element that
-- names a skill, op, or flow). Rename the skill-name columns; step_index
-- columns keep their names — they are flow positions.
ALTER TABLE terminal_sessions RENAME COLUMN step TO skill;
ALTER TABLE run_events RENAME COLUMN step TO skill;
