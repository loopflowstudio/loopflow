-- name: remove_task_lifecycle_outcome
-- id: f6a698c6969573c74a7c3016dec3bc88
-- depends_on:
ALTER TABLE tasks DROP COLUMN lifecycle_outcome;
