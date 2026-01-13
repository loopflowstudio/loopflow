"""Tests for maestro functionality."""

import uuid
from datetime import datetime
from pathlib import Path

from loopflow.maestro import (
    Session,
    SessionStatus,
)


def test_session_serialization():
    """Session can be serialized and deserialized."""
    session = Session(
        id=str(uuid.uuid4()),
        task="test",
        repo=Path("/project"),
        worktree=Path("/project/worktree"),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=12345,
    )

    data = session.to_dict()
    assert data["task"] == "test"
    assert data["status"] == "running"
    assert data["run_mode"] == "auto"

    restored = Session.from_dict(data)
    assert restored.task == session.task
    assert restored.status == session.status


def test_session_from_dict_handles_model_field():
    """Session.from_dict maps model -> backend."""
    data = {
        "id": str(uuid.uuid4()),
        "task": "test",
        "repo": "/project",
        "worktree": "/project/worktree",
        "status": "running",
        "started_at": datetime.now().isoformat(),
        "model": "codex",
    }

    session = Session.from_dict(data)
    assert session.backend == "codex"


def test_session_status_enum():
    """SessionStatus has expected values."""
    assert SessionStatus.RUNNING.value == "running"
    assert SessionStatus.WAITING.value == "waiting"
    assert SessionStatus.COMPLETED.value == "completed"
    assert SessionStatus.ERROR.value == "error"
