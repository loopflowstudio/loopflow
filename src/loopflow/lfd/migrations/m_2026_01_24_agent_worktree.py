"""Add name, worktree, branch, and pending_activations columns to waves table.

Waves have a name used for worktree/branch naming. main_branch is now computed as {name}.main.
Waves maintain a single persistent worktree, cycling through branches via move_worktree.
"""

import sqlite3

VERSION = "2026-01-24T00:00:00Z"
DESCRIPTION = "Add name, worktree, branch, and pending_activations to waves"


def apply(conn: sqlite3.Connection) -> None:
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1] for row in cursor.fetchall()}

    if "name" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN name TEXT NOT NULL DEFAULT ''")

    if "worktree" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN worktree TEXT")

    if "branch" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN branch TEXT")

    if "pending_activations" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN pending_activations INTEGER NOT NULL DEFAULT 0")
