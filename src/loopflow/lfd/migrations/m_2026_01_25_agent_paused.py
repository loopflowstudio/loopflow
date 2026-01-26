"""Add paused column to waves table."""

VERSION = "2026_01_25_wave_paused"
DESCRIPTION = "Add paused column to waves"


def apply(conn) -> None:
    """Add paused column (default 0 = False = automatic mode)."""
    # Check if column already exists (baseline now includes it)
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1] for row in cursor.fetchall()}
    if "paused" in columns:
        return  # Already exists

    cursor = conn.cursor()
    cursor.execute("ALTER TABLE waves ADD COLUMN paused INTEGER DEFAULT 0")
    conn.commit()
