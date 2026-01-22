"""
Rename loops → jobs, loop_runs → job_runs
"""

import sqlite3

VERSION = "2025-01-22T00:00:00"
DESCRIPTION = "Rename loops to jobs"


def apply(conn: sqlite3.Connection) -> None:
    # Check if jobs table already exists (migration already applied)
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='jobs'")
    if cursor.fetchone():
        return  # Already migrated

    # Rename tables
    conn.execute("ALTER TABLE loops RENAME TO jobs")
    conn.execute("ALTER TABLE loop_runs RENAME TO job_runs")

    # Rename columns
    # SQLite 3.25.0+ supports ALTER TABLE RENAME COLUMN
    conn.execute("ALTER TABLE jobs RENAME COLUMN loop_main TO job_main")
    conn.execute("ALTER TABLE job_runs RENAME COLUMN loop_id TO job_id")

    # Drop old indexes
    conn.execute("DROP INDEX IF EXISTS idx_loops_area_repo")
    conn.execute("DROP INDEX IF EXISTS idx_loops_repo")
    conn.execute("DROP INDEX IF EXISTS idx_loops_status")
    conn.execute("DROP INDEX IF EXISTS idx_loop_runs_loop")

    # Create new indexes
    conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_area_repo ON jobs(type, area, repo)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_jobs_repo ON jobs(repo)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_job_runs_job ON job_runs(job_id)")
