"""
Create runs and triggers schema (Loop, Subscription, Schedule).

Migrates from old loops/loop_runs tables if they exist.
"""

import sqlite3

VERSION = "2025-01-22T02:00:00"
DESCRIPTION = "Create runs and triggers schema"


def apply(conn: sqlite3.Connection) -> None:
    # Check if new schema already exists
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='runs'")
    if cursor.fetchone():
        return  # Already migrated

    # Check if old schema exists and needs migration
    # We need to rename old tables BEFORE creating new ones
    cursor = conn.execute("PRAGMA table_info(loops)")
    columns = {row[1] for row in cursor.fetchall()}

    old_loops_exists = len(columns) > 0
    old_schema = "type" in columns  # Old schema has 'type' column

    if old_loops_exists and old_schema:
        # Rename old tables to preserve data for migration
        conn.execute("ALTER TABLE loops RENAME TO loops_old")
        cursor = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='loop_runs'"
        )
        if cursor.fetchone():
            conn.execute("ALTER TABLE loop_runs RENAME TO loop_runs_old")

    # Create new tables
    conn.executescript(
        """
        -- Runs: execution instances
        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            parent TEXT,  -- "loop:<id>" | "subscription:<id>" | "schedule:<id>" | NULL

            flow TEXT NOT NULL,
            area TEXT NOT NULL,
            repo TEXT NOT NULL,
            goals TEXT,  -- JSON array

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

        CREATE INDEX IF NOT EXISTS idx_runs_parent ON runs(parent);
        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
        CREATE INDEX IF NOT EXISTS idx_runs_repo ON runs(repo);

        -- Loops: continuous runners
        CREATE TABLE IF NOT EXISTS loops (
            id TEXT PRIMARY KEY,
            flow TEXT NOT NULL,
            area TEXT NOT NULL,
            repo TEXT NOT NULL,
            goals TEXT,

            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            main_branch TEXT NOT NULL,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_loops_area_repo ON loops(area, repo);
        CREATE INDEX IF NOT EXISTS idx_loops_repo ON loops(repo);
        CREATE INDEX IF NOT EXISTS idx_loops_status ON loops(status);

        -- Subscriptions: pathset watchers
        CREATE TABLE IF NOT EXISTS subscriptions (
            id TEXT PRIMARY KEY,
            flow TEXT NOT NULL,
            area TEXT NOT NULL,
            repo TEXT NOT NULL,
            goals TEXT,

            pathset TEXT NOT NULL,
            last_main_sha TEXT,

            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            main_branch TEXT NOT NULL,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_area_repo ON subscriptions(area, repo);
        CREATE INDEX IF NOT EXISTS idx_subscriptions_repo ON subscriptions(repo);

        -- Schedules: cron triggers
        CREATE TABLE IF NOT EXISTS schedules (
            id TEXT PRIMARY KEY,
            flow TEXT NOT NULL,
            area TEXT NOT NULL,
            repo TEXT NOT NULL,
            goals TEXT,

            cron TEXT NOT NULL,

            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            main_branch TEXT NOT NULL,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_schedules_area_repo ON schedules(area, repo);
        CREATE INDEX IF NOT EXISTS idx_schedules_repo ON schedules(repo);
        """
    )

    # Migrate data from old schema if it exists
    if old_loops_exists and old_schema:
        _migrate_old_data(conn)


def _migrate_old_data(conn: sqlite3.Connection) -> None:
    """Migrate data from old loops_old/loop_runs_old tables."""
    old_table = "loops_old"
    old_runs_table = "loop_runs_old"

    # Migrate triggers based on type
    cursor = conn.execute(f"SELECT * FROM {old_table}")
    for row in cursor.fetchall():
        row_dict = dict(zip([d[0] for d in cursor.description], row))
        trigger_type = row_dict.get("type", "loop")

        # Get main_branch from old schema
        main_branch = row_dict.get("loop_main") or ""

        if trigger_type == "loop":
            conn.execute(
                """
                INSERT INTO loops (id, flow, area, repo, goals, status, iteration,
                    main_branch, pr_limit, merge_mode, pid, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    row_dict["id"],
                    row_dict.get("flow") or "ship",
                    row_dict["area"],
                    row_dict["repo"],
                    row_dict.get("goals"),
                    row_dict["status"],
                    row_dict.get("iteration", 0),
                    main_branch,
                    row_dict.get("pr_limit", 5),
                    row_dict.get("merge_mode", "pr"),
                    row_dict.get("pid"),
                    row_dict["created_at"],
                ),
            )
            parent = f"loop:{row_dict['id']}"

        elif trigger_type == "subscribe":
            conn.execute(
                """
                INSERT INTO subscriptions (id, flow, area, repo, goals, pathset, last_main_sha,
                    status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    row_dict["id"],
                    row_dict.get("flow") or "ship",
                    row_dict["area"],
                    row_dict["repo"],
                    row_dict.get("goals"),
                    row_dict.get("pathset", ""),
                    row_dict.get("last_main_sha"),
                    row_dict["status"],
                    row_dict.get("iteration", 0),
                    main_branch,
                    row_dict.get("pr_limit", 5),
                    row_dict.get("merge_mode", "pr"),
                    row_dict.get("pid"),
                    row_dict["created_at"],
                ),
            )
            parent = f"subscription:{row_dict['id']}"

        elif trigger_type == "schedule":
            conn.execute(
                """
                INSERT INTO schedules (id, flow, area, repo, goals, cron,
                    status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    row_dict["id"],
                    row_dict.get("flow") or "ship",
                    row_dict["area"],
                    row_dict["repo"],
                    row_dict.get("goals"),
                    row_dict.get("cron", ""),
                    row_dict["status"],
                    row_dict.get("iteration", 0),
                    main_branch,
                    row_dict.get("pr_limit", 5),
                    row_dict.get("merge_mode", "pr"),
                    row_dict.get("pid"),
                    row_dict["created_at"],
                ),
            )
            parent = f"schedule:{row_dict['id']}"

        elif trigger_type == "flow":
            # Flow type = direct run with no parent trigger
            parent = None

        else:
            continue

        # Migrate runs for this trigger
        cursor2 = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            (old_runs_table,),
        )
        if not cursor2.fetchone():
            continue

        runs_cursor = conn.execute(
            f"SELECT * FROM {old_runs_table} WHERE loop_id = ?", (row_dict["id"],)
        )
        for run_row in runs_cursor.fetchall():
            run_dict = dict(zip([d[0] for d in runs_cursor.description], run_row))

            # Map old status to new status
            old_status = run_dict["status"]
            new_status = {
                "idle": "completed",
                "running": "running",
                "waiting": "pending",
                "error": "failed",
            }.get(old_status, "completed")

            conn.execute(
                """
                INSERT INTO runs (id, parent, flow, area, repo, goals, status, iteration,
                    worktree, current_step, error, pr_url, started_at, ended_at, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    run_dict["id"],
                    parent,
                    row_dict.get("flow") or "ship",
                    row_dict["area"],
                    row_dict["repo"],
                    row_dict.get("goals"),
                    new_status,
                    run_dict.get("iteration", 0),
                    run_dict.get("worktree"),
                    run_dict.get("current_step"),
                    run_dict.get("error"),
                    run_dict.get("pr_url"),
                    run_dict["started_at"],
                    run_dict.get("ended_at"),
                    run_dict["started_at"],  # Use started_at as created_at
                ),
            )

    # Drop old tables
    conn.execute(f"DROP TABLE IF EXISTS {old_runs_table}")
    conn.execute(f"DROP TABLE IF EXISTS {old_table}")
