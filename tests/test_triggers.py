"""Tests for trigger evaluation."""

from datetime import datetime, timedelta
from pathlib import Path
from unittest.mock import patch

from loopflow.maestro.markdown import AgentFile
from loopflow.maestro.triggers import (
    _cron_matches,
    _matches_field,
    should_trigger,
)


def _make_agent(trigger: str = "manual", cron: str | None = None) -> AgentFile:
    return AgentFile(
        name="test-agent",
        path=Path("/tmp/test.md"),
        repo=Path("/tmp/repo"),
        pipeline=["ship"],
        trigger=trigger,
        context=[],
        prompt="Test",
        cron=cron,
    )


# _matches_field tests


def test_matches_field_star():
    assert _matches_field("*", 0) is True
    assert _matches_field("*", 59) is True
    assert _matches_field("*", 23) is True


def test_matches_field_exact_match():
    assert _matches_field("0", 0) is True
    assert _matches_field("15", 15) is True
    assert _matches_field("59", 59) is True


def test_matches_field_no_match():
    assert _matches_field("0", 1) is False
    assert _matches_field("15", 16) is False


def test_matches_field_step():
    assert _matches_field("*/15", 0) is True
    assert _matches_field("*/15", 15) is True
    assert _matches_field("*/15", 30) is True
    assert _matches_field("*/15", 45) is True
    assert _matches_field("*/15", 7) is False
    assert _matches_field("*/15", 22) is False


def test_matches_field_step_invalid():
    assert _matches_field("*/abc", 0) is False


def test_matches_field_invalid_number():
    assert _matches_field("abc", 0) is False


# _cron_matches tests


def test_cron_matches_none():
    assert _cron_matches(None, None) is False


def test_cron_matches_invalid_format():
    assert _cron_matches("0 9 *", None) is False  # only 3 fields
    assert _cron_matches("0 9 * * * *", None) is False  # 6 fields


def test_cron_matches_all_stars():
    """All stars should always match."""
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 30, 0)
        assert _cron_matches("* * * * *", None) is True


def test_cron_matches_specific_minute():
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 30, 0)
        assert _cron_matches("30 * * * *", None) is True
        assert _cron_matches("31 * * * *", None) is False


def test_cron_matches_specific_hour():
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 30, 0)
        assert _cron_matches("* 9 * * *", None) is True
        assert _cron_matches("* 10 * * *", None) is False


def test_cron_matches_daily_at_9am():
    """0 9 * * * = daily at 9am."""
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 0, 0)
        assert _cron_matches("0 9 * * *", None) is True

        mock_dt.now.return_value = datetime(2024, 1, 15, 10, 0, 0)
        assert _cron_matches("0 9 * * *", None) is False


def test_cron_matches_every_15_minutes():
    """*/15 * * * * = every 15 minutes."""
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 0, 0)
        assert _cron_matches("*/15 * * * *", None) is True

        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 15, 0)
        assert _cron_matches("*/15 * * * *", None) is True

        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 7, 0)
        assert _cron_matches("*/15 * * * *", None) is False


def test_cron_matches_weekday():
    """Check weekday matching (0=Monday)."""
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        # Monday = 0
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 0, 0)  # Monday
        assert _cron_matches("0 9 * * 0", None) is True

        mock_dt.now.return_value = datetime(2024, 1, 16, 9, 0, 0)  # Tuesday
        assert _cron_matches("0 9 * * 0", None) is False


def test_cron_matches_skip_if_ran_recently():
    """Don't trigger if we already ran this minute."""
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        now = datetime(2024, 1, 15, 9, 0, 30)
        mock_dt.now.return_value = now

        # Ran 30 seconds ago - should skip
        last_run = now - timedelta(seconds=30)
        assert _cron_matches("* * * * *", last_run) is False

        # Ran 90 seconds ago - should trigger
        last_run = now - timedelta(seconds=90)
        assert _cron_matches("* * * * *", last_run) is True


# should_trigger tests


def test_should_trigger_manual():
    agent = _make_agent(trigger="manual")
    assert should_trigger(agent, None, None) is False


def test_should_trigger_loop():
    agent = _make_agent(trigger="loop")
    assert should_trigger(agent, None, None) is True


def test_should_trigger_cron():
    agent = _make_agent(trigger="cron", cron="* * * * *")
    with patch("loopflow.maestro.triggers.datetime") as mock_dt:
        mock_dt.now.return_value = datetime(2024, 1, 15, 9, 0, 0)
        assert should_trigger(agent, None, None) is True


def test_should_trigger_cron_no_expression():
    agent = _make_agent(trigger="cron", cron=None)
    assert should_trigger(agent, None, None) is False
