"""Rename trigger/mode columns to stimulus terminology."""

import sqlite3

VERSION = "2026-01-23T17:00:00"
DESCRIPTION = "Rename mode/watch_paths/cron to stimulus_kind/stimulus_cron"


def apply(conn: sqlite3.Connection) -> None:
    # Check if already migrated (stimulus_kind exists)
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1] for row in cursor.fetchall()}

    if "stimulus_kind" in columns:
        return  # Already migrated

    # Add new columns
    conn.execute("ALTER TABLE waves ADD COLUMN stimulus_kind TEXT NOT NULL DEFAULT 'loop'")
    conn.execute("ALTER TABLE waves ADD COLUMN stimulus_cron TEXT")

    # Migrate data from old columns
    # mode -> stimulus_kind
    if "mode" in columns:
        conn.execute("UPDATE waves SET stimulus_kind = mode WHERE mode IS NOT NULL")

    # cron -> stimulus_cron
    if "cron" in columns:
        conn.execute("UPDATE waves SET stimulus_cron = cron WHERE cron IS NOT NULL")

    # watch_paths is removed - area now serves as watch paths
    # No data migration needed; check_watch_stimulus uses wave.area

    conn.commit()
