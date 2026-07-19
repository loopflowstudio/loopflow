-- Promotion occurrence is distinct from chord ancestry. Existing parent links
-- remain ancestry-only and must not wake a child Wave after migration.

ALTER TABLE waves ADD COLUMN promoted_at INTEGER;
