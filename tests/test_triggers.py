"""Tests for trigger evaluation and cron parsing."""

from datetime import datetime, timedelta
from pathlib import Path

from loopflow.lfd.cron import (
    parse_cron,
    matches_schedule,
    should_run_cron,
    _parse_field,
)
from loopflow.lfd.models import AgentSpec, AgentRun, AgentStatus, TriggerSpec, TriggerKind
from loopflow.lfd.triggers import should_trigger


def _make_agent(trigger_kind: TriggerKind = TriggerKind.MANUAL, cron: str | None = None) -> AgentSpec:
    return AgentSpec(
        name="test-agent",
        repo=Path("/tmp/repo"),
        pipeline="ship",
        trigger=TriggerSpec(kind=trigger_kind, cron=cron),
        context=[],
        prompt="Test",
    )


# _parse_field tests


def test_parse_field_star():
    result = _parse_field("*", 0, 59)
    assert 0 in result
    assert 59 in result
    assert len(result) == 60


def test_parse_field_exact_match():
    result = _parse_field("0", 0, 59)
    assert result == [0]

    result = _parse_field("15", 0, 59)
    assert result == [15]


def test_parse_field_step():
    result = _parse_field("*/15", 0, 59)
    assert 0 in result
    assert 15 in result
    assert 30 in result
    assert 45 in result
    assert 7 not in result
    assert 22 not in result


# parse_cron tests


def test_parse_cron_invalid_format():
    try:
        parse_cron("0 9 *")  # only 3 fields
        assert False, "Should raise ValueError"
    except ValueError:
        pass

    try:
        parse_cron("0 9 * * * *")  # 6 fields
        assert False, "Should raise ValueError"
    except ValueError:
        pass


def test_parse_cron_all_stars():
    schedule = parse_cron("* * * * *")
    assert len(schedule.minute) == 60
    assert len(schedule.hour) == 24


def test_parse_cron_specific_minute():
    schedule = parse_cron("30 * * * *")
    assert schedule.minute == [30]


def test_parse_cron_specific_hour():
    schedule = parse_cron("* 9 * * *")
    assert schedule.hour == [9]


# matches_schedule tests


def test_matches_schedule_all_stars():
    """All stars should always match."""
    schedule = parse_cron("* * * * *")
    dt = datetime(2024, 1, 15, 9, 30, 0)
    assert matches_schedule(schedule, dt) is True


def test_matches_schedule_specific_minute():
    schedule = parse_cron("30 * * * *")
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 30, 0)) is True
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 31, 0)) is False


def test_matches_schedule_specific_hour():
    schedule = parse_cron("* 9 * * *")
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 30, 0)) is True
    assert matches_schedule(schedule, datetime(2024, 1, 15, 10, 30, 0)) is False


def test_matches_schedule_daily_at_9am():
    """0 9 * * * = daily at 9am."""
    schedule = parse_cron("0 9 * * *")
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 0, 0)) is True
    assert matches_schedule(schedule, datetime(2024, 1, 15, 10, 0, 0)) is False


def test_matches_schedule_every_15_minutes():
    """*/15 * * * * = every 15 minutes."""
    schedule = parse_cron("*/15 * * * *")
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 0, 0)) is True
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 15, 0)) is True
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 7, 0)) is False


def test_matches_schedule_weekday():
    """Check weekday matching (0=Monday)."""
    schedule = parse_cron("0 9 * * 0")
    # Monday = 0 (datetime.weekday() returns 0 for Monday)
    assert matches_schedule(schedule, datetime(2024, 1, 15, 9, 0, 0)) is True  # Monday
    assert matches_schedule(schedule, datetime(2024, 1, 16, 9, 0, 0)) is False  # Tuesday


# should_run_cron tests


def test_should_run_cron_no_last_run():
    """Without last run, should run if schedule matches now."""
    schedule = parse_cron("* * * * *")
    now = datetime(2024, 1, 15, 9, 0, 0)
    assert should_run_cron(schedule, None, now) is True


def test_should_run_cron_ran_recently():
    """Don't trigger if we already ran this minute."""
    schedule = parse_cron("* * * * *")
    now = datetime(2024, 1, 15, 9, 0, 30)
    last_run = now - timedelta(seconds=30)  # Ran 30 seconds ago
    assert should_run_cron(schedule, last_run, now) is False


def test_should_run_cron_ran_long_ago():
    """Trigger if last run was long enough ago."""
    schedule = parse_cron("* * * * *")
    now = datetime(2024, 1, 15, 9, 0, 0)
    last_run = now - timedelta(minutes=2)  # Ran 2 minutes ago
    assert should_run_cron(schedule, last_run, now) is True


# should_trigger tests


def test_should_trigger_manual():
    agent = _make_agent(trigger_kind=TriggerKind.MANUAL)
    assert should_trigger(agent, None) is False


def test_should_trigger_loop():
    agent = _make_agent(trigger_kind=TriggerKind.LOOP)
    assert should_trigger(agent, None) is True


def test_should_trigger_cron_no_expression():
    agent = _make_agent(trigger_kind=TriggerKind.CRON, cron=None)
    assert should_trigger(agent, None) is False
