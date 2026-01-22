"""Schedule checking for lfd.

Evaluates cron expressions and triggers schedules on time.
Designed for laptop use: missed schedules within 24h still trigger on wake.
"""

from datetime import datetime, timedelta

from croniter import croniter

from loopflow.lfd.db import get_latest_run_for_trigger, list_schedules
from loopflow.lfd.loops import start_schedule
from loopflow.lfd.models import Schedule, TriggerStatus

# Grace period for missed schedules (laptop was asleep/off)
SCHEDULE_GRACE_PERIOD = timedelta(hours=24)


def should_trigger_cron(
    cron_expr: str,
    last_run: datetime | None,
    grace_period: timedelta = SCHEDULE_GRACE_PERIOD,
) -> bool:
    """Check if cron should trigger based on last run time.

    Triggers if:
    - The previous scheduled time is after last_run (a schedule was missed)
    - AND the scheduled time is within the grace period (not too stale)

    This handles laptop use: if computer was off at 9am but wakes at 2pm,
    the 9am schedule still runs. But if computer was off for a week,
    stale schedules are skipped.
    """
    now = datetime.now()
    cron = croniter(cron_expr, now)

    # Get previous scheduled time
    prev_time = cron.get_prev(datetime)

    # Skip if scheduled time is too old (stale)
    if now - prev_time > grace_period:
        return False

    if last_run is None:
        # First check - trigger if we're past the scheduled time (and within grace)
        return True

    # Trigger if prev_time is after last_run
    return prev_time > last_run


def check_schedule(schedule: Schedule) -> bool:
    """Check if schedule should trigger. Returns True if should trigger."""
    if not schedule.cron:
        return False

    # Get last completed run
    last_run = get_latest_run_for_trigger("schedule", schedule.id)
    last_time = last_run.ended_at if last_run else None

    return should_trigger_cron(schedule.cron, last_time)


def run_schedule_check() -> list[str]:
    """Check all schedules and trigger as needed.

    Returns list of schedule IDs that were triggered.
    """
    triggered = []
    for schedule in list_schedules():
        if schedule.status == TriggerStatus.RUNNING:
            continue  # Already running

        if check_schedule(schedule):
            result = start_schedule(schedule.id)
            if result:
                triggered.append(schedule.id)

    return triggered
