"""Subscription checking for lfd.

Monitors file changes on main and triggers subscriptions when pathsets are modified.
"""

import subprocess

from loopflow.lfd.db import list_subscriptions, update_subscription_sha
from loopflow.lfd.loops import start_subscription
from loopflow.lfd.models import Subscription, TriggerStatus


def check_subscription(sub: Subscription) -> bool:
    """Check if subscription should trigger. Returns True if triggered.

    Updates last_main_sha as a side effect.
    """
    if not sub.pathset:
        return False

    repo = sub.repo

    # Fetch main
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)

    # Get current main SHA
    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    current_sha = result.stdout.strip()

    if current_sha == sub.last_main_sha:
        return False  # No change

    if sub.last_main_sha is None:
        # First run - set baseline, don't trigger
        update_subscription_sha(sub.id, current_sha)
        return False

    # Check if pathset was modified
    paths = [p.strip() for p in sub.pathset.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", sub.last_main_sha, current_sha, "--"] + paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        # Main changed but not our paths
        update_subscription_sha(sub.id, current_sha)
        return False

    # Trigger iteration - update SHA before starting
    update_subscription_sha(sub.id, current_sha)
    return True


def run_subscription_check() -> list[str]:
    """Check all subscriptions and trigger as needed.

    Returns list of subscription IDs that were triggered.
    """
    triggered = []
    for sub in list_subscriptions():
        if sub.status == TriggerStatus.RUNNING:
            continue  # Already running

        if check_subscription(sub):
            result = start_subscription(sub.id)
            if result:
                triggered.append(sub.id)

    return triggered
