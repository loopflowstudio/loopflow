"""
Rename sessions table to step_runs, task column to step.

Part of the execution model cleanup:
- Session → StepRun
- task → step
"""

import sqlite3

VERSION = "2025-01-22T04:00:00"
DESCRIPTION = "rename sessions to step_runs, task to step"


def apply(conn: sqlite3.Connection) -> None:
    # Rename table and column
    conn.executescript(
        """
        -- Create new table with correct names
        CREATE TABLE IF NOT EXISTS step_runs (
            id TEXT PRIMARY KEY,
            step TEXT NOT NULL,
            repo TEXT NOT NULL,
            worktree TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            model TEXT NOT NULL,
            run_mode TEXT NOT NULL DEFAULT 'auto'
        );

        -- Copy data from old table if it exists
        INSERT OR IGNORE INTO step_runs
            (id, step, repo, worktree, status, started_at, ended_at, pid, model, run_mode)
        SELECT id, task, repo, worktree, status, started_at, ended_at, pid, model, run_mode
        FROM sessions;

        -- Drop old table and index
        DROP INDEX IF EXISTS idx_sessions_status;
        DROP TABLE IF EXISTS sessions;

        -- Create index on new table
        CREATE INDEX IF NOT EXISTS idx_step_runs_status ON step_runs(status);
        """
    )
