-- Collapse the multi-repo wave model to single-repo: a wave targets exactly
-- one repo. The per-wave, per-repo execution state that lived in `wave_repos`
-- (migration 042) moves back onto `waves` as plain columns, and the table is
-- dropped. Multi-repo waves are deprioritized to a nice-to-have; nothing in the
-- codebase writes more than one `wave_repos` row per wave today.
--
-- Additive ADD COLUMNs (re-run tolerated as "duplicate column name"), then a
-- backfill from `wave_repos`, then the drop.
ALTER TABLE waves ADD COLUMN repo TEXT NOT NULL DEFAULT '';
ALTER TABLE waves ADD COLUMN worktree TEXT NOT NULL DEFAULT '';
ALTER TABLE waves ADD COLUMN branch TEXT NOT NULL DEFAULT '';
ALTER TABLE waves ADD COLUMN status INTEGER NOT NULL DEFAULT 0;
ALTER TABLE waves ADD COLUMN iteration INTEGER NOT NULL DEFAULT 0;
ALTER TABLE waves ADD COLUMN cycle_start_iteration INTEGER NOT NULL DEFAULT 0;

-- Backfill from the primary repo. A wave should only ever have one row, but if
-- history left several, the lowest `position` (then lowest `repo`) wins — the
-- same "primary repo" the old code read via `repos.first()`.
UPDATE waves SET
    repo = COALESCE((
        SELECT wr.repo FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), ''),
    worktree = COALESCE((
        SELECT wr.worktree FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), ''),
    branch = COALESCE((
        SELECT wr.branch FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), ''),
    status = COALESCE((
        SELECT wr.status FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), 0),
    iteration = COALESCE((
        SELECT wr.iteration FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), 0),
    cycle_start_iteration = COALESCE((
        SELECT wr.cycle_start_iteration FROM wave_repos wr WHERE wr.wave_id = waves.id
        ORDER BY wr.position ASC, wr.repo ASC LIMIT 1), 0);

DROP TABLE IF EXISTS wave_repos;
