"""Tests for maestro SQLite database."""

from datetime import datetime
from pathlib import Path
import uuid

from loopflow.maestro.session import Session, SessionStatus
from loopflow.maestro.db import (
    init_db,
    save_session,
    load_sessions,
    update_session_status,
    delete_session,
)


def test_init_db_creates_schema(tmp_path):
    """init_db creates tables and indices."""
    db_path = tmp_path / "test.db"

    init_db(db_path)

    assert db_path.exists()

    # Verify WAL mode
    import sqlite3

    conn = sqlite3.connect(db_path)
    cursor = conn.execute("PRAGMA journal_mode")
    assert cursor.fetchone()[0].lower() == "wal"

    # Verify tables exist
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name = 'sessions'"
    )
    tables = [row[0] for row in cursor.fetchall()]
    assert "sessions" in tables

    conn.close()


def test_save_and_load_session(tmp_path):
    """save_session and load_sessions round-trip."""
    db_path = tmp_path / "test.db"
    session = Session(
        id=str(uuid.uuid4()),
        task="test",
        repo=Path("/project"),
        worktree=Path("/project/worktree"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=12345,
        run_mode="auto",
    )

    save_session(db_path, session)
    sessions = load_sessions(db_path)

    assert len(sessions) == 1
    loaded = sessions[0]
    assert loaded.id == session.id
    assert loaded.task == session.task
    assert loaded.status == SessionStatus.RUNNING
    assert loaded.run_mode == "auto"


def test_load_sessions_filters_by_repo(tmp_path):
    """load_sessions filters by repo when provided."""
    db_path = tmp_path / "test.db"

    session1 = Session(
        id=str(uuid.uuid4()),
        task="task1",
        repo=Path("/project1"),
        worktree=Path("/project1/wt"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    session2 = Session(
        id=str(uuid.uuid4()),
        task="task2",
        repo=Path("/project2"),
        worktree=Path("/project2/wt"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    save_session(db_path, session1)
    save_session(db_path, session2)

    sessions = load_sessions(db_path, repo=Path("/project1"))

    assert len(sessions) == 1
    assert sessions[0].task == "task1"


def test_load_sessions_only_running_and_waiting(tmp_path):
    """load_sessions only returns running and waiting sessions."""
    db_path = tmp_path / "test.db"

    running = Session(
        id=str(uuid.uuid4()),
        task="running",
        repo=Path("/project"),
        worktree=Path("/project/wt"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    completed = Session(
        id=str(uuid.uuid4()),
        task="completed",
        repo=Path("/project"),
        worktree=Path("/project/wt"),
        status=SessionStatus.COMPLETED,
        started_at=datetime.now(),
    )

    save_session(db_path, running)
    save_session(db_path, completed)

    sessions = load_sessions(db_path)

    assert len(sessions) == 1
    assert sessions[0].task == "running"


def test_update_session_status(tmp_path):
    """update_session_status modifies existing session."""
    db_path = tmp_path / "test.db"
    session = Session(
        id=str(uuid.uuid4()),
        task="test",
        repo=Path("/project"),
        worktree=Path("/project/wt"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    save_session(db_path, session)
    updated = update_session_status(db_path, session.id, SessionStatus.COMPLETED)

    assert updated is True

    sessions = load_sessions(db_path)
    assert len(sessions) == 0  # COMPLETED not returned by load_sessions


def test_delete_session(tmp_path):
    """delete_session removes session from database."""
    db_path = tmp_path / "test.db"
    session = Session(
        id=str(uuid.uuid4()),
        task="test",
        repo=Path("/project"),
        worktree=Path("/project/wt"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
    )

    save_session(db_path, session)
    deleted = delete_session(db_path, session.id)

    assert deleted is True

    sessions = load_sessions(db_path)
    assert len(sessions) == 0

