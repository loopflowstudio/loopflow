"""Add activation queue columns to agents table.

Agents can queue activations when busy (RUNNING/WAITING).
- pending_activations: count of queued activations
- buffer_mode: "combine" (default) or "queue"
"""

import sqlite3

VERSION = "2026-01-23T20:45:00Z"
DESCRIPTION = "Add activation queue to agents"


def apply(conn: sqlite3.Connection) -> None:
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1] for row in cursor.fetchall()}

    if "pending_activations" not in columns:
        conn.execute(
            "ALTER TABLE agents ADD COLUMN pending_activations INTEGER NOT NULL DEFAULT 0"
        )

    if "buffer_mode" not in columns:
        conn.execute(
            "ALTER TABLE agents ADD COLUMN buffer_mode TEXT NOT NULL DEFAULT 'combine'"
        )
