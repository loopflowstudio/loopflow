"""Add base_branch and base_commit columns to waves table for stacking support.

Waves can now track which branch they're stacked on (base_branch) and the commit SHA
at branch time (base_commit). This enables squash-aware rebasing when the base PR lands.
"""

import sqlite3

VERSION = "2026-01-28T00:00:00Z"
DESCRIPTION = "Add base_branch and base_commit to waves for stacking"


def apply(conn: sqlite3.Connection) -> None:
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1] for row in cursor.fetchall()}

    if "base_branch" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN base_branch TEXT")

    if "base_commit" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN base_commit TEXT")
