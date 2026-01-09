"""Tests for maestro functionality."""

import os
import time
import uuid
from datetime import datetime
from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.maestro import (
    Session,
    SessionStatus,
    NotificationEvent,
    NotificationType,
    connect_maestro,
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

    restored = Session.from_dict(data)
    assert restored.task == session.task
    assert restored.status == session.status


def test_notification_event_serialization():
    """NotificationEvent can be serialized and deserialized."""
    event = NotificationEvent(
        type=NotificationType.COMPLETED,
        session_id="abc123",
        message="Task completed",
        timestamp=datetime.now(),
        repo=Path("/project"),
    )

    data = event.to_dict()
    assert data["type"] == "completed"
    assert data["message"] == "Task completed"

    restored = NotificationEvent.from_dict(data)
    assert restored.type == event.type
    assert restored.message == event.message


def test_connect_maestro_returns_none_when_not_running(tmp_path):
    """connect_maestro returns None when daemon is not running."""
    with patch("loopflow.maestro.client.Path") as mock_path:
        mock_path.home.return_value = tmp_path
        client = connect_maestro()
        assert client is None


def test_session_status_enum():
    """SessionStatus has expected values."""
    assert SessionStatus.RUNNING.value == "running"
    assert SessionStatus.WAITING.value == "waiting"
    assert SessionStatus.COMPLETED.value == "completed"
    assert SessionStatus.ERROR.value == "error"


def test_notification_type_enum():
    """NotificationType has expected values."""
    assert NotificationType.STARTED.value == "started"
    assert NotificationType.WAITING.value == "waiting"
    assert NotificationType.COMPLETED.value == "completed"
    assert NotificationType.ERROR.value == "error"
