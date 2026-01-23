"""
Create unified agents table, migrate from loops/subscriptions/schedules.
"""

import json
import sqlite3

VERSION = "2025-01-22T03:00:00"
DESCRIPTION = "Create unified agents table"


def apply(conn: sqlite3.Connection) -> None:
    # Check if agents table already exists
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='agents'")
    if cursor.fetchone():
        return  # Already migrated

    # Create agents table
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS agents (
            id TEXT PRIMARY KEY,
            repo TEXT NOT NULL,
            flow TEXT NOT NULL,
            voice TEXT,  -- JSON array
            area TEXT,   -- JSON array

            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER NOT NULL DEFAULT 0,

            main_branch TEXT NOT NULL,
            pr_limit INTEGER NOT NULL DEFAULT 5,
            merge_mode TEXT NOT NULL DEFAULT 'pr',

            pid INTEGER,
            created_at TEXT NOT NULL,

            -- Activation config
            watch_paths TEXT,
            cron TEXT,
            last_main_sha TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_agents_repo ON agents(repo);
        CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
        """
    )

    # Migrate data from old tables
    _migrate_loops(conn)
    _migrate_subscriptions(conn)
    _migrate_schedules(conn)
    _migrate_runs(conn)

    # Drop old tables
    conn.execute("DROP TABLE IF EXISTS loops")
    conn.execute("DROP TABLE IF EXISTS subscriptions")
    conn.execute("DROP TABLE IF EXISTS schedules")


def _migrate_loops(conn: sqlite3.Connection) -> None:
    """Migrate loops to agents."""
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='loops'")
    if not cursor.fetchone():
        return

    cursor = conn.execute("SELECT * FROM loops")
    for row in cursor.fetchall():
        row_dict = dict(zip([d[0] for d in cursor.description], row))

        # Convert area string to JSON array
        area = row_dict.get("area", ".")
        area_json = json.dumps([area])

        # Goals become voice
        goals_str = row_dict.get("goals")
        voice = json.loads(goals_str) if goals_str else ["default"]
        voice_json = json.dumps(voice)

        conn.execute(
            """
            INSERT INTO agents (id, repo, flow, voice, area, status, iteration,
                main_branch, pr_limit, merge_mode, pid, created_at,
                watch_paths, cron, last_main_sha)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
            """,
            (
                row_dict["id"],
                row_dict["repo"],
                row_dict["flow"],
                voice_json,
                area_json,
                row_dict["status"],
                row_dict.get("iteration", 0),
                row_dict.get("main_branch", ""),
                row_dict.get("pr_limit", 5),
                row_dict.get("merge_mode", "pr"),
                row_dict.get("pid"),
                row_dict["created_at"],
            ),
        )


def _migrate_subscriptions(conn: sqlite3.Connection) -> None:
    """Migrate subscriptions to agents."""
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='subscriptions'"
    )
    if not cursor.fetchone():
        return

    cursor = conn.execute("SELECT * FROM subscriptions")
    for row in cursor.fetchall():
        row_dict = dict(zip([d[0] for d in cursor.description], row))

        area = row_dict.get("area", ".")
        area_json = json.dumps([area])

        goals_str = row_dict.get("goals")
        voice = json.loads(goals_str) if goals_str else ["default"]
        voice_json = json.dumps(voice)

        conn.execute(
            """
            INSERT INTO agents (id, repo, flow, voice, area, status, iteration,
                main_branch, pr_limit, merge_mode, pid, created_at,
                watch_paths, cron, last_main_sha)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?)
            """,
            (
                row_dict["id"],
                row_dict["repo"],
                row_dict["flow"],
                voice_json,
                area_json,
                row_dict["status"],
                row_dict.get("iteration", 0),
                row_dict.get("main_branch", ""),
                row_dict.get("pr_limit", 5),
                row_dict.get("merge_mode", "pr"),
                row_dict.get("pid"),
                row_dict["created_at"],
                row_dict.get("pathset"),  # watch_paths
                row_dict.get("last_main_sha"),
            ),
        )


def _migrate_schedules(conn: sqlite3.Connection) -> None:
    """Migrate schedules to agents."""
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='schedules'")
    if not cursor.fetchone():
        return

    cursor = conn.execute("SELECT * FROM schedules")
    for row in cursor.fetchall():
        row_dict = dict(zip([d[0] for d in cursor.description], row))

        area = row_dict.get("area", ".")
        area_json = json.dumps([area])

        goals_str = row_dict.get("goals")
        voice = json.loads(goals_str) if goals_str else ["default"]
        voice_json = json.dumps(voice)

        conn.execute(
            """
            INSERT INTO agents (id, repo, flow, voice, area, status, iteration,
                main_branch, pr_limit, merge_mode, pid, created_at,
                watch_paths, cron, last_main_sha)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL)
            """,
            (
                row_dict["id"],
                row_dict["repo"],
                row_dict["flow"],
                voice_json,
                area_json,
                row_dict["status"],
                row_dict.get("iteration", 0),
                row_dict.get("main_branch", ""),
                row_dict.get("pr_limit", 5),
                row_dict.get("merge_mode", "pr"),
                row_dict.get("pid"),
                row_dict["created_at"],
                row_dict.get("cron"),
            ),
        )


def _migrate_runs(conn: sqlite3.Connection) -> None:
    """Update runs table: rename parent to agent, convert format."""
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='runs'")
    if not cursor.fetchone():
        return

    # Check if runs table has parent column
    cursor = conn.execute("PRAGMA table_info(runs)")
    columns = {row[1] for row in cursor.fetchall()}

    if "parent" not in columns:
        return  # Already migrated or different schema

    if "agent" in columns:
        return  # Already has agent column

    # Rebuild runs table with new schema
    conn.executescript(
        """
        CREATE TABLE runs_new (
            id TEXT PRIMARY KEY,
            agent TEXT,

            flow TEXT NOT NULL,
            voice TEXT,
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
        """
    )

    # Migrate data with transformations
    cursor = conn.execute("SELECT * FROM runs")
    for row in cursor.fetchall():
        row_dict = dict(zip([d[0] for d in cursor.description], row))

        # Extract agent ID from parent (e.g., "loop:uuid" -> "uuid")
        parent = row_dict.get("parent")
        agent = None
        if parent:
            if ":" in parent:
                agent = parent.split(":", 1)[1]
            else:
                agent = parent

        # Convert area string to JSON array
        area = row_dict.get("area", ".")
        area_json = json.dumps([area])

        # Goals become voice
        goals_str = row_dict.get("goals")
        voice = json.loads(goals_str) if goals_str else ["default"]
        voice_json = json.dumps(voice)

        conn.execute(
            """
            INSERT INTO runs_new (id, agent, flow, voice, area, repo, status, iteration,
                worktree, branch, current_step, error, pr_url, started_at, ended_at, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                row_dict["id"],
                agent,
                row_dict["flow"],
                voice_json,
                area_json,
                row_dict["repo"],
                row_dict["status"],
                row_dict.get("iteration", 0),
                row_dict.get("worktree"),
                row_dict.get("branch"),
                row_dict.get("current_step"),
                row_dict.get("error"),
                row_dict.get("pr_url"),
                row_dict.get("started_at"),
                row_dict.get("ended_at"),
                row_dict["created_at"],
            ),
        )

    conn.executescript(
        """
        DROP TABLE runs;
        ALTER TABLE runs_new RENAME TO runs;

        CREATE INDEX IF NOT EXISTS idx_runs_agent ON runs(agent);
        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
        CREATE INDEX IF NOT EXISTS idx_runs_repo ON runs(repo);
        """
    )
