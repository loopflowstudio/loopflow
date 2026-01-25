"""Add paused column to agents table."""

VERSION = "2026_01_25_agent_paused"
DESCRIPTION = "Add paused column to agents"


def apply(conn) -> None:
    """Add paused column (default 1 = True = manual mode)."""
    cursor = conn.cursor()
    cursor.execute("ALTER TABLE agents ADD COLUMN paused INTEGER DEFAULT 1")
    conn.commit()
