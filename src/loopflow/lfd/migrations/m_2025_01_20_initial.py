"""
initial schema
"""

import sqlite3

VERSION = "2025-01-20T00:00:00"
DESCRIPTION = "initial schema"


def apply(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS loops (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            area TEXT NOT NULL,
            repo TEXT NOT NULL,
            loop_main TEXT NOT NULL,
            goals TEXT,
            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER DEFAULT 0,
            pr_limit INTEGER DEFAULT 5,
            merge_mode TEXT DEFAULT 'pr',
            project_file TEXT,
            pathset TEXT,
            cron TEXT,
            goal TEXT,
            pid INTEGER,
            last_main_sha TEXT,
            created_at TEXT NOT NULL,
            flow TEXT
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_loops_area_repo
            ON loops(type, area, repo);
        CREATE INDEX IF NOT EXISTS idx_loops_repo ON loops(repo);
        CREATE INDEX IF NOT EXISTS idx_loops_status ON loops(status);

        CREATE TABLE IF NOT EXISTS loop_runs (
            id TEXT PRIMARY KEY,
            loop_id TEXT NOT NULL,
            iteration INTEGER NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            worktree TEXT,
            current_step TEXT,
            error TEXT,
            pr_url TEXT,
            FOREIGN KEY (loop_id) REFERENCES loops(id)
        );

        CREATE INDEX IF NOT EXISTS idx_loop_runs_loop ON loop_runs(loop_id);

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            task TEXT NOT NULL,
            repo TEXT NOT NULL,
            worktree TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            model TEXT NOT NULL,
            run_mode TEXT NOT NULL DEFAULT 'auto'
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
        """
    )
