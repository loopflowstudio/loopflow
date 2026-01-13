"""Tests for agent daemon."""

import os
import signal
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.maestro.db import get_db


def _init_runs_table(db_path: Path) -> None:
    """Initialize the agent_runs table."""
    conn = get_db(db_path)
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS agent_runs (
            id TEXT PRIMARY KEY,
            agent_name TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            worktree TEXT,
            iteration INTEGER DEFAULT 0,
            error TEXT,
            main_sha TEXT
        );
    """)
    conn.commit()
    conn.close()


def _insert_run(db_path: Path, agent_name: str, pid: int, status: str = "running") -> str:
    """Insert a test run record."""
    run_id = str(uuid.uuid4())
    conn = get_db(db_path)
    conn.execute(
        """INSERT INTO agent_runs (id, agent_name, status, started_at, pid)
           VALUES (?, ?, ?, ?, ?)""",
        (run_id, agent_name, status, datetime.now().isoformat(), pid),
    )
    conn.commit()
    conn.close()
    return run_id


def _get_run(db_path: Path, run_id: str) -> dict | None:
    """Get a run record."""
    conn = get_db(db_path)
    cursor = conn.execute("SELECT * FROM agent_runs WHERE id = ?", (run_id,))
    row = cursor.fetchone()
    conn.close()
    return dict(row) if row else None


def test_update_completed_runs_marks_dead_process(tmp_path, monkeypatch):
    """_update_completed_runs marks completed when process is gone."""
    from loopflow.maestro import daemon

    db_path = tmp_path / "test.db"
    monkeypatch.setattr(daemon, "DEFAULT_DB_PATH", db_path)

    _init_runs_table(db_path)

    # Use a PID that doesn't exist
    fake_pid = 99999999
    run_id = _insert_run(db_path, "test-agent", fake_pid)

    # Verify it's running
    run = _get_run(db_path, run_id)
    assert run["status"] == "running"
    assert run["ended_at"] is None

    # Update completed runs
    daemon._update_completed_runs()

    # Verify it's now completed
    run = _get_run(db_path, run_id)
    assert run["status"] == "completed"
    assert run["ended_at"] is not None


def test_update_completed_runs_leaves_running_process(tmp_path, monkeypatch):
    """_update_completed_runs leaves running processes alone."""
    from loopflow.maestro import daemon

    db_path = tmp_path / "test.db"
    monkeypatch.setattr(daemon, "DEFAULT_DB_PATH", db_path)

    _init_runs_table(db_path)

    # Use current process PID - guaranteed to be running
    run_id = _insert_run(db_path, "test-agent", os.getpid())

    # Update completed runs
    daemon._update_completed_runs()

    # Verify it's still running
    run = _get_run(db_path, run_id)
    assert run["status"] == "running"
    assert run["ended_at"] is None


def test_update_completed_runs_ignores_already_completed(tmp_path, monkeypatch):
    """_update_completed_runs ignores already completed runs."""
    from loopflow.maestro import daemon

    db_path = tmp_path / "test.db"
    monkeypatch.setattr(daemon, "DEFAULT_DB_PATH", db_path)

    _init_runs_table(db_path)

    # Insert an already completed run
    run_id = _insert_run(db_path, "test-agent", 99999999, status="completed")

    # Update completed runs
    daemon._update_completed_runs()

    # Verify it's unchanged
    run = _get_run(db_path, run_id)
    assert run["status"] == "completed"
