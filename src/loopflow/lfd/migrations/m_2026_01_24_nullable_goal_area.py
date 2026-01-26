"""Make direction and area nullable in waves table.

Per lfd-cli-redesign: area/direction are optional at create-time, validated at run-time.
"""

import sqlite3

VERSION = "2026-01-24T22:00:00Z"
DESCRIPTION = "make direction and area nullable"


def apply(conn: sqlite3.Connection) -> None:
    """SQLite doesn't support ALTER COLUMN, so recreate the table."""
    # Check if migration is needed by checking if direction allows NULL
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1]: row for row in cursor.fetchall()}

    # row format: (cid, name, type, notnull, dflt_value, pk)
    direction_notnull = columns.get("direction", (None,) * 6)[3]
    if direction_notnull == 0:
        # Already nullable, skip
        return

    conn.executescript(
        """
        -- Create new table with nullable direction/area
        CREATE TABLE waves_new (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            repo TEXT NOT NULL,
            flow TEXT NOT NULL,
            direction TEXT,             -- NOW NULLABLE
            area TEXT,             -- NOW NULLABLE

            stimulus_kind TEXT NOT NULL DEFAULT 'loop',
            stimulus_cron TEXT,
            paused INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            worktree TEXT,
            branch TEXT,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL,

            last_main_sha TEXT,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            pending_activations INTEGER NOT NULL DEFAULT 0
        );

        -- Copy data
        INSERT INTO waves_new
        SELECT id, name, repo, flow, direction, area,
               stimulus_kind, stimulus_cron, paused, status, iteration,
               worktree, branch, pr_limit, merge_mode,
               pid, created_at, last_main_sha,
               consecutive_failures, pending_activations
        FROM waves;

        -- Drop old table and rename
        DROP TABLE waves;
        ALTER TABLE waves_new RENAME TO waves;

        -- Recreate indexes
        CREATE INDEX IF NOT EXISTS idx_waves_repo ON waves(repo);
        CREATE INDEX IF NOT EXISTS idx_waves_status ON waves(status);
        """
    )
