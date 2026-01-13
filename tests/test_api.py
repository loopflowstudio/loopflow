"""Tests for maestro HTTP API."""

from datetime import datetime, timedelta

from loopflow.maestro.api import _format_elapsed


def test_format_elapsed_seconds():
    """_format_elapsed shows seconds for recent times."""
    started_at = datetime.now() - timedelta(seconds=30)

    result = _format_elapsed(started_at)

    assert result == "30s"


def test_format_elapsed_minutes():
    """_format_elapsed shows minutes for 1-59 minutes."""
    started_at = datetime.now() - timedelta(minutes=5)

    result = _format_elapsed(started_at)

    assert result == "5m"


def test_format_elapsed_hours():
    """_format_elapsed shows hours for 1-23 hours."""
    started_at = datetime.now() - timedelta(hours=3)

    result = _format_elapsed(started_at)

    assert result == "3h"


def test_format_elapsed_days():
    """_format_elapsed shows days for 24+ hours."""
    started_at = datetime.now() - timedelta(days=2)

    result = _format_elapsed(started_at)

    assert result == "2d"


def test_format_elapsed_zero():
    """_format_elapsed handles just-started sessions."""
    started_at = datetime.now()

    result = _format_elapsed(started_at)

    assert result == "0s"


def test_format_elapsed_boundary_59_seconds():
    """_format_elapsed shows seconds up to 59."""
    started_at = datetime.now() - timedelta(seconds=59)

    result = _format_elapsed(started_at)

    assert result == "59s"


def test_format_elapsed_boundary_60_seconds():
    """_format_elapsed switches to minutes at 60 seconds."""
    started_at = datetime.now() - timedelta(seconds=60)

    result = _format_elapsed(started_at)

    assert result == "1m"


def test_format_elapsed_boundary_59_minutes():
    """_format_elapsed shows minutes up to 59."""
    started_at = datetime.now() - timedelta(minutes=59)

    result = _format_elapsed(started_at)

    assert result == "59m"


def test_format_elapsed_boundary_60_minutes():
    """_format_elapsed switches to hours at 60 minutes."""
    started_at = datetime.now() - timedelta(minutes=60)

    result = _format_elapsed(started_at)

    assert result == "1h"


def test_format_elapsed_boundary_23_hours():
    """_format_elapsed shows hours up to 23."""
    started_at = datetime.now() - timedelta(hours=23)

    result = _format_elapsed(started_at)

    assert result == "23h"


def test_format_elapsed_boundary_24_hours():
    """_format_elapsed switches to days at 24 hours."""
    started_at = datetime.now() - timedelta(hours=24)

    result = _format_elapsed(started_at)

    assert result == "1d"
