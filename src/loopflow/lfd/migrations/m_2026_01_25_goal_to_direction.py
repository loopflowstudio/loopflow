"""Rename goal column to direction.

Terminology change: goal -> direction.
"""

import sqlite3

DESCRIPTION = "rename goal to direction"


def apply(conn: sqlite3.Connection) -> None:
    """Rename goal column to direction in waves and runs tables."""
    # SQLite doesn't support RENAME COLUMN before 3.25, so we recreate the tables

    # Agents table
    conn.executescript("""
        -- Rename goal to direction in waves
        CREATE TABLE waves_new (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            repo TEXT NOT NULL,
            flow TEXT NOT NULL DEFAULT 'design',
            direction TEXT,            -- was: goal
            area TEXT,

            stimulus TEXT NOT NULL DEFAULT '{"type":"loop"}',
            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            worktree TEXT,
            branch TEXT,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',
            paused INTEGER NOT NULL DEFAULT 0,

            pid INTEGER,
            created_at TEXT NOT NULL,

            watch_paths TEXT,
            cron TEXT,
            last_main_sha TEXT,

            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            pending_activations INTEGER NOT NULL DEFAULT 0
        );

        INSERT INTO waves_new
        SELECT id, name, repo, flow, goal, area, stimulus, status, iteration,
               worktree, branch, pr_limit, merge_mode, paused, pid, created_at,
               watch_paths, cron, last_main_sha, consecutive_failures, pending_activations
        FROM waves;

        DROP TABLE waves;
        ALTER TABLE waves_new RENAME TO waves;

        CREATE INDEX IF NOT EXISTS idx_waves_repo ON waves(repo);
        CREATE INDEX IF NOT EXISTS idx_waves_status ON waves(status);
    """)

    # Runs table
    conn.executescript("""
        -- Rename goal to direction in runs
        CREATE TABLE runs_new (
            id TEXT PRIMARY KEY,
            agent TEXT,

            flow TEXT NOT NULL,
            direction TEXT,            -- was: goal
            area TEXT,
            repo TEXT NOT NULL,

            status TEXT NOT NULL DEFAULT 'pending',
            iteration INTEGER NOT NULL DEFAULT 0,

            worktree TEXT,
            branch TEXT,
            current_step TEXT,
            error TEXT,
            pr_url TEXT,

            started_at TEXT,
            ended_at TEXT,
            created_at TEXT NOT NULL
        );

        INSERT INTO runs_new
        SELECT id, agent, flow, goal, area, repo, status, iteration,
               worktree, branch, current_step, error, pr_url,
               started_at, ended_at, created_at
        FROM runs;

        DROP TABLE runs;
        ALTER TABLE runs_new RENAME TO runs;

        CREATE INDEX IF NOT EXISTS idx_runs_agent ON runs(agent);
        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
        CREATE INDEX IF NOT EXISTS idx_runs_repo ON runs(repo);
    """)
