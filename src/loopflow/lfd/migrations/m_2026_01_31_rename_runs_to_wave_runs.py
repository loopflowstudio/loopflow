"""Rename runs table to wave_runs and flow_run_id to wave_run_id."""

import sqlite3

SCHEMA_VERSION = "2026-01-31T12:00:00Z_rename_runs_to_wave_runs"
DESCRIPTION = "rename runs to wave_runs, flow_run_id to wave_run_id"


def apply(conn: sqlite3.Connection) -> None:
    # Check if the old 'runs' table exists
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='runs'")
    if cursor.fetchone():
        # Rename the table
        conn.execute("ALTER TABLE runs RENAME TO wave_runs")

        # Rename the 'wave' column to 'wave_id' (SQLite 3.25+)
        conn.execute("ALTER TABLE wave_runs RENAME COLUMN wave TO wave_id")

        # Drop old indexes and create new ones
        conn.execute("DROP INDEX IF EXISTS idx_runs_wave")
        conn.execute("DROP INDEX IF EXISTS idx_runs_status")
        conn.execute("DROP INDEX IF EXISTS idx_runs_repo")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_wave_runs_wave ON wave_runs(wave_id)")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_wave_runs_status ON wave_runs(status)")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_wave_runs_repo ON wave_runs(repo)")

    # Check if the agents table has the old 'flow_run_id' column
    cursor = conn.execute("PRAGMA table_info(agents)")
    columns = {row[1] for row in cursor.fetchall()}

    if "flow_run_id" in columns:
        # Rename the column
        conn.execute("ALTER TABLE agents RENAME COLUMN flow_run_id TO wave_run_id")

        # Drop old index and create new one
        conn.execute("DROP INDEX IF EXISTS idx_agents_flow_run")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_agents_wave_run ON agents(wave_run_id)")
