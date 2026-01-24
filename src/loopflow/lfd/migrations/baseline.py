"""Baseline schema - compressed from all prior migrations."""

import sqlite3

SCHEMA_VERSION = "2026-01-23T16:39:43Z_b159a91"
DESCRIPTION = "baseline schema"


def apply(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        -- Agents: AI coding agents with loop/watch/cron modes
        CREATE TABLE IF NOT EXISTS agents (
            id TEXT PRIMARY KEY,
            repo TEXT NOT NULL,
            flow TEXT NOT NULL,
            goal TEXT NOT NULL,  -- JSON array
            area TEXT NOT NULL,   -- JSON array

            mode TEXT NOT NULL DEFAULT 'loop',  -- loop, watch, cron
            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            main_branch TEXT NOT NULL,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL,

            -- Trigger config (watch/cron modes)
            watch_paths TEXT,     -- comma-separated paths for watch mode
            cron TEXT,            -- cron expression for cron mode
            last_main_sha TEXT,   -- last seen SHA on main (watch mode)

            -- Resilience
            consecutive_failures INTEGER NOT NULL DEFAULT 0
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_agents_area_repo ON agents(area, repo);
        CREATE INDEX IF NOT EXISTS idx_agents_repo ON agents(repo);
        CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

        -- Runs: flow execution instances
        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            agent TEXT,  -- agent ID (nullable for one-off runs)

            flow TEXT NOT NULL,
            goal TEXT NOT NULL,  -- JSON array
            area TEXT NOT NULL,   -- JSON array
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

        CREATE INDEX IF NOT EXISTS idx_runs_agent ON runs(agent);
        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
        CREATE INDEX IF NOT EXISTS idx_runs_repo ON runs(repo);

        -- Step runs: individual step executions
        CREATE TABLE IF NOT EXISTS step_runs (
            id TEXT PRIMARY KEY,
            step TEXT NOT NULL,
            repo TEXT NOT NULL,
            worktree TEXT NOT NULL,

            flow_run_id TEXT,  -- parent run (nullable for standalone)
            agent_id TEXT,     -- parent agent (nullable for standalone)

            status TEXT NOT NULL DEFAULT 'running',
            started_at TEXT NOT NULL,
            ended_at TEXT,

            pid INTEGER,
            model TEXT NOT NULL DEFAULT 'claude-code',
            run_mode TEXT NOT NULL DEFAULT 'auto'
        );

        CREATE INDEX IF NOT EXISTS idx_step_runs_status ON step_runs(status);
        CREATE INDEX IF NOT EXISTS idx_step_runs_flow_run ON step_runs(flow_run_id);

        -- Summaries: cached codebase summaries
        CREATE TABLE IF NOT EXISTS summaries (
            id TEXT PRIMARY KEY,
            repo TEXT NOT NULL,
            path TEXT NOT NULL,
            token_budget INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            content TEXT NOT NULL,
            model TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_summaries_repo_path ON summaries(repo, path);
        """
    )
