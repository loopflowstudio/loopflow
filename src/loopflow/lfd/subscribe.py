"""Subscription checking for lfd.

Monitors file changes on main and triggers jobs when pathsets are modified.
"""

import subprocess

from loopflow.lfd.db import list_jobs, update_job_last_sha
from loopflow.lfd.jobs import start_job
from loopflow.lfd.models import Job, JobStatus, TriggerType


def check_subscription(job: Job) -> bool:
    """Check if subscription should trigger. Returns True if triggered.

    Updates last_main_sha as a side effect.
    Caller must ensure job.trigger_type == TriggerType.PATHSET.
    """
    if not job.pathset:
        return False

    repo = job.repo

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

    if current_sha == job.last_main_sha:
        return False  # No change

    if job.last_main_sha is None:
        # First run - set baseline, don't trigger
        update_job_last_sha(job.id, current_sha)
        return False

    # Check if pathset was modified
    paths = [p.strip() for p in job.pathset.split(",")]
    result = subprocess.run(
        ["git", "diff", "--name-only", job.last_main_sha, current_sha, "--"] + paths,
        cwd=repo,
        capture_output=True,
        text=True,
    )

    changed_files = result.stdout.strip()
    if not changed_files:
        # Main changed but not our paths
        update_job_last_sha(job.id, current_sha)
        return False

    # Trigger iteration - update SHA before starting
    update_job_last_sha(job.id, current_sha)
    return True


def run_subscription_check() -> list[str]:
    """Check all subscriptions and trigger as needed.

    Returns list of job IDs that were triggered.
    """
    triggered = []
    for job in list_jobs():
        if job.trigger_type != TriggerType.PATHSET:
            continue
        if job.status == JobStatus.RUNNING:
            continue  # Already running

        if check_subscription(job):
            result = start_job(job.id)
            if result:
                triggered.append(job.id)

    return triggered
