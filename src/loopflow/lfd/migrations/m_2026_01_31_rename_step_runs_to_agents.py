"""Rename step_runs table to agents for consistency with Rust lfd."""

import sqlite3

VERSION = "2026-01-31T00:00:00Z"
DESCRIPTION = "Rename step_runs table to agents"


def apply(conn: sqlite3.Connection) -> None:
    # Check if step_runs exists and agents doesn't
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='step_runs'"
    )
    if cursor.fetchone():
        conn.execute("ALTER TABLE step_runs RENAME TO agents")

        # Rename indexes
        conn.execute("DROP INDEX IF EXISTS idx_step_runs_status")
        conn.execute("DROP INDEX IF EXISTS idx_step_runs_flow_run")

        conn.execute("CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status)")
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agents_wave_run ON agents(wave_run_id)"
        )
