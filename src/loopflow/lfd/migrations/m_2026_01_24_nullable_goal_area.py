"""Make goal and area nullable in agents table.

Per lfd-cli-redesign: area/goal are optional at create-time, validated at run-time.
"""

import sqlite3

VERSION = "2026-01-24T22:00:00Z"
DESCRIPTION = "make goal and area nullable"


def apply(conn: sqlite3.Connection) -> None:
    """SQLite doesn't support ALTER COLUMN, so recreate the table."""
    # Check if migration is needed by checking if goal allows NULL
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1]: row for row in cursor.fetchall()}

    # row format: (cid, name, type, notnull, dflt_value, pk)
    goal_notnull = columns.get("goal", (None,) * 6)[3]
    if goal_notnull == 0:
        # Already nullable, skip
        return

    conn.executescript(
        """
        -- Create new table with nullable goal/area
        CREATE TABLE agents_new (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            repo TEXT NOT NULL,
            flow TEXT NOT NULL,
            goal TEXT,             -- NOW NULLABLE
            area TEXT,             -- NOW NULLABLE

            stimulus_kind TEXT NOT NULL DEFAULT 'loop',
            stimulus_cron TEXT,
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
        INSERT INTO agents_new
        SELECT id, name, repo, flow, goal, area,
               stimulus_kind, stimulus_cron, status, iteration,
               worktree, branch, pr_limit, merge_mode,
               pid, created_at, last_main_sha,
               consecutive_failures, pending_activations
        FROM agents;

        -- Drop old table and rename
        DROP TABLE agents;
        ALTER TABLE agents_new RENAME TO agents;

        -- Recreate indexes
        CREATE INDEX IF NOT EXISTS idx_agents_repo ON agents(repo);
        CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
        """
    )
