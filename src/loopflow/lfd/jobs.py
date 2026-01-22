"""Job management for lfd."""

import os
import random
import signal
import subprocess
import sys
import uuid
from pathlib import Path

from loopflow.lf.context import find_worktree_root
from loopflow.lfd.db import (
    get_job,
    get_job_by_area_repo,
    save_job,
    update_job_pid,
    update_job_status,
)
from loopflow.lfd.models import Job, JobStatus, JobType, area_to_slug
from loopflow.lfd.process import is_process_running

# Word lists for generating unique branch names (matches swift/Concerto/NameGenerator.swift)
MAGICAL = [
    "aurora",
    "cascade",
    "crystal",
    "drift",
    "echo",
    "ember",
    "fern",
    "flume",
    "frost",
    "glade",
    "grove",
    "haze",
    "ivy",
    "jade",
    "luna",
    "mist",
    "nova",
    "opal",
    "petal",
    "prism",
    "rain",
    "ripple",
    "sage",
    "shade",
    "spark",
    "star",
    "stone",
    "storm",
    "tide",
    "vale",
    "wave",
    "wisp",
    "wren",
    "zephyr",
]

MUSICAL = [
    "allegro",
    "aria",
    "ballad",
    "cadence",
    "canon",
    "chord",
    "coda",
    "duet",
    "forte",
    "fugue",
    "harmony",
    "hymn",
    "lilt",
    "lyric",
    "melody",
    "motif",
    "opus",
    "prelude",
    "refrain",
    "rondo",
    "sonata",
    "tempo",
    "trill",
    "tune",
    "verse",
    "waltz",
]


def _generate_random_words() -> str:
    """Generate a random magical-musical pair like 'aurora-melody'."""
    magical = random.choice(MAGICAL)
    musical = random.choice(MUSICAL)
    return f"{magical}-{musical}"


def _branch_exists(repo: Path, branch: str) -> bool:
    """Check if a branch exists locally or on origin."""
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/heads/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    if result.returncode == 0:
        return True
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/remotes/origin/{branch}"],
        cwd=repo,
        capture_output=True,
    )
    return result.returncode == 0


def _allocate_job_main(repo: Path, area: str) -> str:
    """Return unique branch name based on area.

    Format: area-words-main
    - With area: concerto-swift-river-main
    - Root: root-swift-river-main
    """
    slug = area_to_slug(area)

    # Try random word combinations until we find an available branch
    for _ in range(100):
        words = _generate_random_words()
        candidate = f"{slug}-{words}-main"
        if not _branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not allocate personal-main branch for {slug}")


def _create_job_main_branch(repo: Path, branch: str) -> None:
    """Create personal-main branch from origin/main if it doesn't exist."""
    if _branch_exists(repo, branch):
        return
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo, capture_output=True)
    result = subprocess.run(
        ["git", "branch", branch, "origin/main"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        subprocess.run(
            ["git", "branch", branch, "main"],
            cwd=repo,
            capture_output=True,
        )


def create_job(
    job_type: JobType,
    area: str,
    repo: Path,
    goals: list[str] | None = None,
    flow: str | None = None,
    project_file: str | None = None,
    pathset: str | None = None,
    cron: str | None = None,
) -> Job:
    """Create or get an existing job for an area+repo combination."""
    goals = goals or []

    # Check if job already exists for this area
    existing = get_job_by_area_repo(job_type, area, repo)
    if existing:
        # Update goals/flow if changed
        if set(existing.goals) != set(goals):
            existing.goals = goals
        if flow and existing.flow != flow:
            existing.flow = flow
        if existing.project_file != project_file:
            existing.project_file = project_file
        if existing.pathset != pathset:
            existing.pathset = pathset
        if existing.cron != cron:
            existing.cron = cron
        save_job(existing)
        return existing

    # Allocate and create personal-main branch based on area
    job_main = _allocate_job_main(repo, area)
    _create_job_main_branch(repo, job_main)

    job = Job(
        id=str(uuid.uuid4()),
        type=job_type,
        area=area,
        repo=repo,
        job_main=job_main,
        flow=flow,
        goals=goals,
        status=JobStatus.IDLE,
        project_file=project_file,
        pathset=pathset,
        cron=cron,
    )

    save_job(job)
    return job


def count_outstanding(job: Job) -> int:
    """Count commits on personal-main ahead of main.

    Returns number of commits on personal-main that are not yet on main.
    """
    # Ensure we have latest refs
    subprocess.run(
        ["git", "fetch", "origin", "main", job.job_main],
        cwd=job.repo,
        capture_output=True,
    )

    # Count commits ahead
    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{job.job_main}"],
        cwd=job.repo,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        return 0  # Branch doesn't exist yet or other error

    try:
        return int(result.stdout.strip())
    except ValueError:
        return 0


class StartResult:
    """Result of attempting to start a job."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_job(job_id: str, foreground: bool = False) -> StartResult:
    """Mark a job as running and start execution.

    If foreground=True, runs the job in the current process.
    Otherwise spawns a background subprocess.

    Returns a StartResult indicating success or failure reason.
    """
    job = get_job(job_id)
    if not job:
        return StartResult(False, "not_found")

    # Check if already running
    if job.status == JobStatus.RUNNING and job.pid and is_process_running(job.pid):
        return StartResult(False, "already_running")

    # Check outstanding commits before starting
    outstanding = count_outstanding(job)
    if outstanding >= job.pr_limit:
        update_job_status(job_id, JobStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    if foreground:
        # Run directly in this process
        update_job_status(job_id, JobStatus.RUNNING)
        update_job_pid(job_id, os.getpid())
        _run_job(job)
        return StartResult(True)
    else:
        # Spawn background process
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.job_runner", job_id],
            cwd=job.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_job_status(job_id, JobStatus.RUNNING)
        update_job_pid(job_id, proc.pid)
        return StartResult(True)


def stop_job(job_id: str, force: bool = False) -> bool:
    """Stop a running job.

    If force=True, sends SIGKILL. Otherwise sends SIGTERM for graceful shutdown.
    """
    job = get_job(job_id)
    if not job:
        return False

    # Kill process if running
    if job.pid and is_process_running(job.pid):
        sig = signal.SIGKILL if force else signal.SIGTERM
        try:
            os.kill(job.pid, sig)
        except OSError:
            pass

    update_job_status(job_id, JobStatus.IDLE)
    update_job_pid(job_id, None)
    return True


def _run_job(job: Job) -> None:
    """Run the job execution until it should pause."""
    from loopflow.lfd.job_runner import run_job_iterations

    run_job_iterations(job)


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()
