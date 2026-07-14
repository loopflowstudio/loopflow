-- Repair ledgers migrated by a pre-release build of 054_step_to_skill that
-- renamed run_events.step_index alongside step -> skill. The landed 054 keeps
-- step_index (it is a flow position, not a skill name), so those ledgers carry
-- a skill_index column that no reader selects, and every run_events query
-- fails with "no such column: step_index".
--
-- Ledgers that never saw the pre-release build have no skill_index; this
-- migration fails there with "no such column" and converges via
-- RENAME_CONVERGENCE_MIGRATIONS.
ALTER TABLE run_events RENAME COLUMN skill_index TO step_index;
