-- Per-repo execution state for waves. Additive: `waves` columns stay intact
-- and nothing reads this table yet. Backfills one row per wave from the
-- existing single-repo columns.
CREATE TABLE wave_repos (
    wave_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL DEFAULT '',
    branch TEXT NOT NULL DEFAULT '',
    status INTEGER NOT NULL DEFAULT 0,
    iteration INTEGER NOT NULL DEFAULT 0,
    cycle_start_iteration INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (wave_id, repo)
);

INSERT INTO wave_repos (wave_id, repo, status, iteration, cycle_start_iteration, position)
SELECT id, repo, status, iteration, cycle_start_iteration, 0 FROM waves;
