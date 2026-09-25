-- name: remove_landing_repair_counter
-- id: bf1b3becbfb981f1da1c08ad5e286d2a
-- depends_on: pr_landings

ALTER TABLE pr_landings DROP COLUMN repair_count;
