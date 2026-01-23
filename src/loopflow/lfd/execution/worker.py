"""Worker for continuous execution.

Runs iterations of a trigger until stopped or paused.
Coordinates with the daemon manager for global concurrency limits.
"""

import json
import socket
import sys
import time
import uuid
from pathlib import Path

from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.execution.runner import run_iteration
from loopflow.lfd.models import Loop, TriggerStatus
from loopflow.lfd.runs.loop import (
    count_outstanding,
    get_loop,
    update_loop_iteration,
    update_loop_pid,
    update_loop_status,
)
from loopflow.lfd.runs.schedule import get_schedule, update_schedule_status
from loopflow.lfd.runs.subscription import get_subscription, update_subscription_status

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"
MANAGER_POLL_INTERVAL = 30  # seconds between slot checks


def _manager_call(method: str, params: dict | None = None) -> dict | None:
    """Make a synchronous call to the daemon manager.

    Returns the result dict on success, None on connection failure.
    """
    if not SOCKET_PATH.exists():
        return None

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5.0)
        sock.connect(str(SOCKET_PATH))

        request = {"method": method}
        if params:
            request["params"] = params

        sock.sendall((json.dumps(request) + "\n").encode())

        response_data = b""
        while b"\n" not in response_data:
            chunk = sock.recv(1024)
            if not chunk:
                break
            response_data += chunk

        sock.close()

        if response_data:
            response = json.loads(response_data.decode().strip())
            if response.get("ok"):
                return response.get("result", {})
        return None
    except Exception:
        return None


def _manager_acquire(run_id: str) -> tuple[bool, str | None]:
    """Try to acquire a manager slot.

    Returns (acquired, reason) when the daemon is available.
    """
    result = _manager_call("scheduler.acquire", {"run_id": run_id})
    if result is None:
        # Daemon not running, allow iteration (standalone mode)
        return True, None
    return result.get("acquired", False), result.get("reason")


def _manager_release(run_id: str) -> None:
    """Release a manager slot."""
    _manager_call("scheduler.release", {"run_id": run_id})


def run_loop_iterations(loop: Loop) -> None:
    """Run loop iterations until PR limit is reached or error occurs."""
    while True:
        outstanding = count_outstanding(loop)
        if outstanding >= loop.pr_limit:
            update_loop_status(loop.id, TriggerStatus.WAITING)
            notify_event(
                "loop.waiting",
                {
                    "loop_id": loop.id,
                    "area": loop.area,
                    "outstanding": outstanding,
                    "limit": loop.pr_limit,
                },
            )
            break

        iteration = loop.iteration + 1
        run_id = str(uuid.uuid4())

        # Wait for manager slot (global concurrency)
        while True:
            acquired, reason = _manager_acquire(run_id)
            if acquired:
                break
            notify_event(
                "scheduler.waiting",
                {
                    "loop_id": loop.id,
                    "area": loop.area,
                    "reason": reason or "concurrency",
                },
            )
            time.sleep(MANAGER_POLL_INTERVAL)

        try:
            success = run_iteration(loop, iteration, run_id)
            if success:
                update_loop_iteration(loop.id, iteration)
                loop.iteration = iteration
            else:
                update_loop_status(loop.id, TriggerStatus.ERROR)
                break
        except Exception as e:
            notify_event(
                "loop.error",
                {
                    "loop_id": loop.id,
                    "area": loop.area,
                    "error": str(e),
                },
            )
            update_loop_status(loop.id, TriggerStatus.ERROR)
            break
        finally:
            _manager_release(run_id)

    update_loop_pid(loop.id, None)


def _trigger_to_loop(trigger) -> Loop:
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
    """Entry point for background worker."""
    if len(sys.argv) < 2:
        print("Usage: python -m loopflow.lfd.execution.worker <type> [id]", file=sys.stderr)
        sys.exit(1)

    trigger_type = sys.argv[1]

    if trigger_type == "loop":
        if len(sys.argv) != 3:
            print("Usage: python -m loopflow.lfd.execution.worker loop <loop_id>", file=sys.stderr)
            sys.exit(1)

        loop_id = sys.argv[2]
        loop = get_loop(loop_id)

        if not loop:
            print(f"Loop not found: {loop_id}", file=sys.stderr)
            sys.exit(1)

        run_loop_iterations(loop)

    elif trigger_type == "subscription":
        if len(sys.argv) != 3:
            print(
                "Usage: python -m loopflow.lfd.execution.worker subscription <id>", file=sys.stderr
            )
            sys.exit(1)

        subscription_id = sys.argv[2]
        success = run_subscription_iteration(subscription_id)
        sys.exit(0 if success else 1)

    elif trigger_type == "schedule":
        if len(sys.argv) != 3:
            print("Usage: python -m loopflow.lfd.execution.worker schedule <id>", file=sys.stderr)
            sys.exit(1)

        schedule_id = sys.argv[2]
        success = run_schedule_iteration(schedule_id)
        sys.exit(0 if success else 1)

    else:
        print(f"Unknown trigger type: {trigger_type}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
