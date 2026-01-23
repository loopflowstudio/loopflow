"""
Add consecutive_failures column to agents table for circuit breaker.
"""

import sqlite3

VERSION = "2025-01-23T01:00:00"
DESCRIPTION = "Add consecutive_failures to agents"


def apply(conn: sqlite3.Connection) -> None:
    # Check if column already exists
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1] for row in cursor.fetchall()}

    if "consecutive_failures" in columns:
        return  # Already migrated

    conn.execute("ALTER TABLE agents ADD COLUMN consecutive_failures INTEGER DEFAULT 0")
