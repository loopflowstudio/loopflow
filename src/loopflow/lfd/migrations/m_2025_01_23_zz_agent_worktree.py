"""
Add worktree and branch columns to agents table.

Agents maintain a single persistent worktree, cycling through branches via `next`.
"""

import sqlite3

VERSION = "2025-01-23T05:00:00"
DESCRIPTION = "Add worktree and branch to agents"


def apply(conn: sqlite3.Connection) -> None:
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1] for row in cursor.fetchall()}

    if "worktree" not in columns:
        conn.execute("ALTER TABLE agents ADD COLUMN worktree TEXT")

    if "branch" not in columns:
        conn.execute("ALTER TABLE agents ADD COLUMN branch TEXT")
