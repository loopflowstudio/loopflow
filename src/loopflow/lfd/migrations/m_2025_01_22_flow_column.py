"""Add flow column to loops table."""

import sqlite3

VERSION = "2025-01-22T01:00:00"
DESCRIPTION = "add flow column to loops"


def apply(conn: sqlite3.Connection) -> None:
    # Check if loops table exists
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='loops'")
    if not cursor.fetchone():
        return  # Table doesn't exist; initial migration will create it with all columns

    # Check if column already exists
    cursor = conn.execute("PRAGMA table_info(loops)")
    columns = {row[1] for row in cursor.fetchall()}

    if "flow" not in columns:
        conn.execute("ALTER TABLE loops ADD COLUMN flow TEXT")
