"""Trigger runner for subscriptions and schedules.

Runs a single iteration for a subscription or schedule trigger.
"""

import sys

from loopflow.lfd.db import (
    get_schedule,
    get_subscription,
    update_schedule_status,
    update_subscription_status,
)
from loopflow.lfd.loop_runner import run_iteration
from loopflow.lfd.models import Loop, Schedule, Subscription, TriggerStatus


def _trigger_to_loop(trigger: Subscription | Schedule) -> Loop:
    """Convert a subscription or schedule to a Loop for run_iteration."""
    return Loop(
        id=trigger.id,
        flow=trigger.flow,
        area=trigger.area,
        repo=trigger.repo,
        goals=trigger.goals,
        status=trigger.status,
        iteration=trigger.iteration,
        main_branch=trigger.main_branch,
        pr_limit=trigger.pr_limit,
        merge_mode=trigger.merge_mode,
        pid=trigger.pid,
        created_at=trigger.created_at,
    )


def run_subscription_iteration(subscription_id: str) -> bool:
    """Run a single iteration for a subscription."""
    sub = get_subscription(subscription_id)
    if not sub:
        return False

    loop = _trigger_to_loop(sub)
    iteration = sub.iteration + 1

    try:
        return run_iteration(loop, iteration, parent_type="subscription")
    finally:
        update_subscription_status(subscription_id, TriggerStatus.IDLE)


def run_schedule_iteration(schedule_id: str) -> bool:
    """Run a single iteration for a schedule."""
    schedule = get_schedule(schedule_id)
    if not schedule:
        return False

    loop = _trigger_to_loop(schedule)
    iteration = schedule.iteration + 1

    try:
        return run_iteration(loop, iteration, parent_type="schedule")
    finally:
        update_schedule_status(schedule_id, TriggerStatus.IDLE)


def main() -> None:
    """Entry point for trigger runner."""
    if len(sys.argv) != 3:
        print("Usage: python -m loopflow.lfd.trigger_runner <type> <id>", file=sys.stderr)
        sys.exit(1)

    trigger_type = sys.argv[1]
    trigger_id = sys.argv[2]

    if trigger_type == "subscription":
        success = run_subscription_iteration(trigger_id)
    elif trigger_type == "schedule":
        success = run_schedule_iteration(trigger_id)
    else:
        print(f"Unknown trigger type: {trigger_type}", file=sys.stderr)
        sys.exit(1)

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
