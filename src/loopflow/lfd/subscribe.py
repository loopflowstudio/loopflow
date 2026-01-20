"""Subscription checking for lfd.

Monitors file changes on main and triggers loops when pathsets are modified.
"""

import subprocess
from pathlib import Path

from loopflow.lfd.db import list_loops, update_loop_last_sha
from loopflow.lfd.loops import start_loop
from loopflow.lfd.models import Loop, LoopStatus, LoopType


def check_subscription(loop: Loop) -> bool:
    """Check if subscription should trigger. Returns True if triggered.

    Updates last_main_sha as a side effect.
    Caller must ensure loop.type == LoopType.SUBSCRIBE.
    """
    if not loop.pathset:
        return False

    repo = loop.repo

    # Fetch main
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)

    # Get current main SHA
    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return False

    current_sha = result.stdout.strip()

    if current_sha == loop.last_main_sha:
        return False  # No change

    if loop.last_main_sha is None:
        # First run - set baseline, don't trigger
        update_loop_last_sha(loop.id, current_sha)
        return False

    # Check if pathset was modified
    paths = [p.strip() for p in loop.pathset.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", loop.last_main_sha, current_sha, "--"] + paths,
        cwd=repo, capture_output=True, text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        # Main changed but not our paths
        update_loop_last_sha(loop.id, current_sha)
        return False

    # Trigger iteration - update SHA before starting
    update_loop_last_sha(loop.id, current_sha)
    return True


def run_subscription_check() -> list[str]:
    """Check all subscriptions and trigger as needed.

    Returns list of loop IDs that were triggered.
    """
    triggered = []
    for loop in list_loops():
        if loop.type != LoopType.SUBSCRIBE:
            continue
        if loop.status == LoopStatus.RUNNING:
            continue  # Already running

        if check_subscription(loop):
            result = start_loop(loop.id)
            if result:
                triggered.append(loop.id)

    return triggered
