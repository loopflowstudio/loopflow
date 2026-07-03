-- Drop the legacy per-repo execution columns from `waves`. Per-repo
-- worktree/branch/status/iteration now live only in `wave_repos`; the wave
-- carries identity plus the wave-level `paused` flag. Wave names become
-- globally unique (the old `repo` scoping is gone).
--
-- `repo` is referenced by idx_waves_repo, so drop the index before the column
-- (SQLite refuses to drop an indexed column).
DROP INDEX idx_waves_repo;

ALTER TABLE waves DROP COLUMN repo;
ALTER TABLE waves DROP COLUMN status;
ALTER TABLE waves DROP COLUMN iteration;
ALTER TABLE waves DROP COLUMN cycle_start_iteration;

-- Global wave-name uniqueness: replace the plain name index with a unique one.
DROP INDEX idx_waves_name;
CREATE UNIQUE INDEX idx_waves_name ON waves(name);
