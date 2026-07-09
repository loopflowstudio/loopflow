-- Repair ledgers migrated by a pre-release build of 054_step_to_skill that
-- renamed runs.step_index even though it is a flow position, not a skill name.
ALTER TABLE runs RENAME COLUMN skill_index TO step_index;
