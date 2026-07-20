-- name: wave_promotion_occurrence
-- id: c45c464ab7eaa7c7f305c9a219bb707a
-- depends_on: task_feedback_reviewers

-- Promotion occurrence is distinct from chord ancestry. Existing parent links
-- remain ancestry-only and must not wake a child Wave after migration.

ALTER TABLE waves ADD COLUMN promoted_at INTEGER;
