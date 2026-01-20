"""Schedule checking for lfd.

Evaluates cron expressions and triggers loops on schedule.
"""

from datetime import datetime

from croniter import croniter

from loopflow.lfd.db import get_latest_loop_run, list_loops
from loopflow.lfd.loops import start_loop
from loopflow.lfd.models import Loop, LoopStatus, LoopType


def should_trigger_cron(cron_expr: str, last_run: datetime | None) -> bool:
    """Check if cron should trigger based on last run time."""
    now = datetime.now()
    cron = croniter(cron_expr, now)

    # Get previous scheduled time
    prev_time = cron.get_prev(datetime)

    if last_run is None:
        # First check - trigger if we're past the scheduled time
        return True

    # Trigger if prev_time is after last_run
    return prev_time > last_run


def check_schedule(loop: Loop) -> bool:
    """Check if scheduled loop should trigger. Returns True if should trigger."""
    if loop.type != LoopType.SCHEDULE:
        return False

    if not loop.cron:
        return False

    # Get last completed run
    last_run = get_latest_loop_run(loop.id)
    last_time = last_run.ended_at if last_run else None

    return should_trigger_cron(loop.cron, last_time)


def run_schedule_check() -> list[str]:
    """Check all schedules and trigger as needed.

    Returns list of loop IDs that were triggered.
    """
    triggered = []
    for loop in list_loops():
        if loop.type != LoopType.SCHEDULE:
            continue
        if loop.status == LoopStatus.RUNNING:
            continue  # Already running

        if check_schedule(loop):
            result = start_loop(loop.id)
            if result:
                triggered.append(loop.id)

    return triggered
