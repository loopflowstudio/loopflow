"""Add step_index column to runs table for tick-based flow execution."""

import sqlite3

SCHEMA_VERSION = "2026-01-26T00:00:00Z_step_index"
DESCRIPTION = "Add step_index to runs for interactive flow steps"


def apply(conn: sqlite3.Connection) -> None:
    # Check if column already exists (baseline schema may have it)
    cursor = conn.execute("PRAGMA table_info(runs)")
    columns = {row[1] for row in cursor.fetchall()}

    if "step_index" not in columns:
        conn.execute("ALTER TABLE runs ADD COLUMN step_index INTEGER NOT NULL DEFAULT 0")
