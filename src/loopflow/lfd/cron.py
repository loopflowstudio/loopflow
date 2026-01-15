"""Cron expression parsing and scheduling."""

from dataclasses import dataclass
from datetime import datetime, timedelta


@dataclass
class CronSchedule:
    """Parsed cron schedule (standard 5-field format: min hour day month weekday)."""
    minute: list[int]
    hour: list[int]
    day: list[int]
    month: list[int]
    weekday: list[int]


def parse_cron(expr: str) -> CronSchedule:
    """Parse cron expression (standard 5-field format)."""
    parts = expr.strip().split()
    if len(parts) != 5:
        raise ValueError(f"Invalid cron expression: expected 5 fields, got {len(parts)}")

    return CronSchedule(
        minute=_parse_field(parts[0], 0, 59),
        hour=_parse_field(parts[1], 0, 23),
        day=_parse_field(parts[2], 1, 31),
        month=_parse_field(parts[3], 1, 12),
        weekday=_parse_field(parts[4], 0, 6),
    )


def _parse_field(field: str, min_val: int, max_val: int) -> list[int]:
    """Parse a single cron field into list of matching values."""
    if field == "*":
        return list(range(min_val, max_val + 1))

    values: set[int] = set()

    for part in field.split(","):
        if "/" in part:
            # Step values: */5 or 0-30/5
            range_part, step = part.split("/")
            step = int(step)
            if range_part == "*":
                start, end = min_val, max_val
            else:
                start, end = _parse_range(range_part, min_val, max_val)
            values.update(range(start, end + 1, step))
        elif "-" in part:
            # Range: 1-5
            start, end = _parse_range(part, min_val, max_val)
            values.update(range(start, end + 1))
        else:
            # Single value
            val = int(part)
            if val < min_val or val > max_val:
                raise ValueError(f"Value {val} out of range [{min_val}, {max_val}]")
            values.add(val)

    return sorted(values)


def _parse_range(part: str, min_val: int, max_val: int) -> tuple[int, int]:
    """Parse a range like '1-5'."""
    start_str, end_str = part.split("-")
    start = int(start_str)
    end = int(end_str)
    if start < min_val or end > max_val or start > end:
        raise ValueError(f"Invalid range {part} for [{min_val}, {max_val}]")
    return start, end


def matches_schedule(schedule: CronSchedule, dt: datetime) -> bool:
    """Check if datetime matches the cron schedule."""
    return (
        dt.minute in schedule.minute
        and dt.hour in schedule.hour
        and dt.day in schedule.day
        and dt.month in schedule.month
        and dt.weekday() in schedule.weekday
    )


def should_run_cron(
    schedule: CronSchedule,
    last_run: datetime | None,
    now: datetime,
    grace_minutes: int = 60,
) -> bool:
    """Check if scheduled time passed (with grace period for laptop sleep)."""
    if last_run is None:
        return matches_schedule(schedule, now)

    # Check each minute from last_run to now (within grace period)
    current = last_run.replace(second=0, microsecond=0)
    grace_start = now.replace(second=0, microsecond=0)

    # Don't look back more than grace_minutes
    earliest = now - timedelta(minutes=grace_minutes)
    if current < earliest:
        current = earliest

    # Check if any scheduled time was missed
    while current <= now:
        if current > last_run and matches_schedule(schedule, current):
            return True
        current += timedelta(minutes=1)

    return False


def next_run_time(schedule: CronSchedule, after: datetime) -> datetime:
    """Calculate next scheduled run time."""
    current = after.replace(second=0, microsecond=0) + timedelta(minutes=1)

    # Limit search to prevent infinite loop
    max_iterations = 366 * 24 * 60  # One year of minutes

    for _ in range(max_iterations):
        if matches_schedule(schedule, current):
            return current
        current += timedelta(minutes=1)

    raise ValueError("Could not find next run time within one year")
