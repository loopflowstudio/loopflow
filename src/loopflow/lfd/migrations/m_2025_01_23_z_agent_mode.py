"""Add explicit mode column to agents table."""

import sqlite3

VERSION = "2025-01-23T04:00:00"
DESCRIPTION = "Add explicit mode column to agents"


def apply(conn: sqlite3.Connection) -> None:
    # Check if mode column already exists
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1] for row in cursor.fetchall()}
    if "mode" in columns:
        return  # Already migrated

    # Add mode column with default 'loop'
    conn.execute("ALTER TABLE agents ADD COLUMN mode TEXT NOT NULL DEFAULT 'loop'")

    # Set mode based on existing trigger config
    conn.execute("UPDATE agents SET mode = 'watch' WHERE watch_paths IS NOT NULL")
    conn.execute("UPDATE agents SET mode = 'cron' WHERE cron IS NOT NULL")
    conn.commit()
