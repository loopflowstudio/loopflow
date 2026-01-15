"""Tests for cron parsing and scheduling."""

from datetime import datetime, timedelta

import pytest

from loopflow.lfd.cron import (
    CronSchedule,
    parse_cron,
    matches_schedule,
    should_run_cron,
    next_run_time,
)


def test_parse_cron_all_stars():
    schedule = parse_cron("* * * * *")
    assert schedule.minute == list(range(60))
    assert schedule.hour == list(range(24))
    assert schedule.day == list(range(1, 32))
    assert schedule.month == list(range(1, 13))
    assert schedule.weekday == list(range(7))


def test_parse_cron_specific_time():
    schedule = parse_cron("30 9 * * *")
    assert schedule.minute == [30]
    assert schedule.hour == [9]


def test_parse_cron_range():
    schedule = parse_cron("0-30 9-17 * * *")
    assert schedule.minute == list(range(31))
    assert schedule.hour == list(range(9, 18))


def test_parse_cron_step():
    schedule = parse_cron("*/15 * * * *")
    assert schedule.minute == [0, 15, 30, 45]


def test_parse_cron_list():
    schedule = parse_cron("0 9,12,18 * * *")
    assert schedule.minute == [0]
    assert schedule.hour == [9, 12, 18]


def test_parse_cron_weekday():
    schedule = parse_cron("0 9 * * 1-5")
    assert schedule.weekday == [1, 2, 3, 4, 5]


def test_parse_cron_invalid_field_count():
    with pytest.raises(ValueError, match="expected 5 fields"):
        parse_cron("* * *")


def test_parse_cron_invalid_value():
    with pytest.raises(ValueError, match="out of range"):
        parse_cron("60 * * * *")


def test_matches_schedule_exact():
    schedule = parse_cron("30 9 * * *")
    dt = datetime(2024, 1, 15, 9, 30, 0)
    assert matches_schedule(schedule, dt)


def test_matches_schedule_no_match_minute():
    schedule = parse_cron("30 9 * * *")
    dt = datetime(2024, 1, 15, 9, 31, 0)
    assert not matches_schedule(schedule, dt)


def test_matches_schedule_no_match_hour():
    schedule = parse_cron("30 9 * * *")
    dt = datetime(2024, 1, 15, 10, 30, 0)
    assert not matches_schedule(schedule, dt)


def test_should_run_cron_first_run():
    schedule = parse_cron("30 9 * * *")
    now = datetime(2024, 1, 15, 9, 30, 0)
    assert should_run_cron(schedule, None, now)


def test_should_run_cron_no_match():
    schedule = parse_cron("30 9 * * *")
    now = datetime(2024, 1, 15, 10, 0, 0)
    assert not should_run_cron(schedule, None, now)


def test_should_run_cron_missed_within_grace():
    schedule = parse_cron("30 9 * * *")
    last_run = datetime(2024, 1, 14, 9, 30, 0)
    now = datetime(2024, 1, 15, 9, 45, 0)
    assert should_run_cron(schedule, last_run, now, grace_minutes=60)


def test_should_run_cron_missed_outside_grace():
    schedule = parse_cron("30 9 * * *")
    last_run = datetime(2024, 1, 14, 9, 30, 0)
    now = datetime(2024, 1, 15, 12, 0, 0)  # 2.5 hours after scheduled time
    # With 60 min grace, looking back only 60 min from now
    # Won't find 9:30 in that window
    assert not should_run_cron(schedule, last_run, now, grace_minutes=60)


def test_next_run_time():
    schedule = parse_cron("30 9 * * *")
    after = datetime(2024, 1, 15, 8, 0, 0)
    next_time = next_run_time(schedule, after)
    assert next_time == datetime(2024, 1, 15, 9, 30, 0)


def test_next_run_time_next_day():
    schedule = parse_cron("30 9 * * *")
    after = datetime(2024, 1, 15, 10, 0, 0)
    next_time = next_run_time(schedule, after)
    assert next_time == datetime(2024, 1, 16, 9, 30, 0)


def test_next_run_time_every_15_minutes():
    schedule = parse_cron("*/15 * * * *")
    after = datetime(2024, 1, 15, 10, 5, 0)
    next_time = next_run_time(schedule, after)
    assert next_time == datetime(2024, 1, 15, 10, 15, 0)
