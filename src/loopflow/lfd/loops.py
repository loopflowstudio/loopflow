"""Loop management for lfd."""

import os
import random
import signal
import subprocess
import sys
import uuid
from pathlib import Path

from loopflow.lf.context import find_worktree_root
from loopflow.lfd.db import (
    get_loop,
    get_loop_by_area_repo,
    get_schedule,
    get_schedule_by_area_repo,
    get_subscription,
    get_subscription_by_area_repo,
    save_loop,
    save_schedule,
    save_subscription,
    update_loop_pid,
    update_loop_status,
    update_schedule_status,
    update_subscription_status,
)
from loopflow.lfd.models import (
    Loop,
    MergeMode,
    Schedule,
    Subscription,
    Trigger,
    TriggerStatus,
    area_to_slug,
)
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


def _allocate_main_branch(repo: Path, area: str) -> str:
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

    raise ValueError(f"Could not allocate main branch for {slug}")


def _create_main_branch(repo: Path, branch: str) -> None:
    """Create main branch from origin/main if it doesn't exist."""
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


def create_loop(
    area: str,
    repo: Path,
    flow: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Loop:
    """Create or get an existing loop for an area+repo combination."""
    goals = goals or []

    # Check if loop already exists for this area
    existing = get_loop_by_area_repo(area, repo)
    if existing:
        # Update if changed
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_loop(existing)
        return existing

    # Allocate and create main branch based on area
    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    loop = Loop(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_loop(loop)
    return loop


def create_subscription(
    area: str,
    repo: Path,
    flow: str,
    pathset: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Subscription:
    """Create or get an existing subscription for an area+repo combination."""
    goals = goals or []

    existing = get_subscription_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.pathset != pathset:
            existing.pathset = pathset
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_subscription(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    sub = Subscription(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        pathset=pathset,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_subscription(sub)
    return sub


def create_schedule(
    area: str,
    repo: Path,
    flow: str,
    cron: str,
    goals: list[str] | None = None,
    pr_limit: int = 5,
    merge_mode: MergeMode = MergeMode.PR,
) -> Schedule:
    """Create or get an existing schedule for an area+repo combination."""
    goals = goals or []

    existing = get_schedule_by_area_repo(area, repo)
    if existing:
        changed = False
        if set(existing.goals) != set(goals):
            existing.goals = goals
            changed = True
        if existing.flow != flow:
            existing.flow = flow
            changed = True
        if existing.cron != cron:
            existing.cron = cron
            changed = True
        if existing.pr_limit != pr_limit:
            existing.pr_limit = pr_limit
            changed = True
        if existing.merge_mode != merge_mode:
            existing.merge_mode = merge_mode
            changed = True
        if changed:
            save_schedule(existing)
        return existing

    main_branch = _allocate_main_branch(repo, area)
    _create_main_branch(repo, main_branch)

    schedule = Schedule(
        id=str(uuid.uuid4()),
        flow=flow,
        area=area,
        repo=repo,
        goals=goals,
        cron=cron,
        status=TriggerStatus.IDLE,
        main_branch=main_branch,
        pr_limit=pr_limit,
        merge_mode=merge_mode,
    )

    save_schedule(schedule)
    return schedule


def count_outstanding(trigger: Trigger) -> int:
    """Count commits on main_branch ahead of main.

    Returns number of commits on main_branch that are not yet on main.
    """
    # Ensure we have latest refs
    subprocess.run(
        ["git", "fetch", "origin", "main", trigger.main_branch],
        cwd=trigger.repo,
        capture_output=True,
    )

    # Count commits ahead
    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{trigger.main_branch}"],
        cwd=trigger.repo,
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
    """Result of attempting to start a trigger."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_loop(loop_id: str, foreground: bool = False) -> StartResult:
    """Start a loop running.

    If foreground=True, runs in the current process.
    Otherwise spawns a background subprocess.
    """
    loop = get_loop(loop_id)
    if not loop:
        return StartResult(False, "not_found")

    # Check if already running
    if loop.status == TriggerStatus.RUNNING and loop.pid and is_process_running(loop.pid):
        return StartResult(False, "already_running")

    # Check outstanding commits before starting
    outstanding = count_outstanding(loop)
    if outstanding >= loop.pr_limit:
        update_loop_status(loop_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    if foreground:
        # Run directly in this process
        update_loop_status(loop_id, TriggerStatus.RUNNING)
        update_loop_pid(loop_id, os.getpid())
        _run_loop(loop)
        return StartResult(True)
    else:
        # Spawn background process
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.loop_runner", loop_id],
            cwd=loop.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_loop_status(loop_id, TriggerStatus.RUNNING)
        update_loop_pid(loop_id, proc.pid)
        return StartResult(True)


def stop_loop(loop_id: str, force: bool = False) -> bool:
    """Stop a running loop.

    If force=True, sends SIGKILL. Otherwise sends SIGTERM for graceful shutdown.
    """
    loop = get_loop(loop_id)
    if not loop:
        return False

    # Kill process if running
    if loop.pid and is_process_running(loop.pid):
        sig = signal.SIGKILL if force else signal.SIGTERM
        try:
            os.kill(loop.pid, sig)
        except OSError:
            pass

    update_loop_status(loop_id, TriggerStatus.IDLE)
    update_loop_pid(loop_id, None)
    return True


def start_subscription(subscription_id: str) -> StartResult:
    """Trigger a subscription to spawn a Run."""
    sub = get_subscription(subscription_id)
    if not sub:
        return StartResult(False, "not_found")

    # Check if already running
    if sub.status == TriggerStatus.RUNNING:
        return StartResult(False, "already_running")

    # Check outstanding commits
    outstanding = count_outstanding(sub)
    if outstanding >= sub.pr_limit:
        update_subscription_status(subscription_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    # Spawn a single iteration
    update_subscription_status(subscription_id, TriggerStatus.RUNNING)
    subprocess.Popen(
        [sys.executable, "-m", "loopflow.lfd.trigger_runner", "subscription", subscription_id],
        cwd=sub.repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return StartResult(True)


def start_schedule(schedule_id: str) -> StartResult:
    """Trigger a schedule to spawn a Run."""
    schedule = get_schedule(schedule_id)
    if not schedule:
        return StartResult(False, "not_found")

    # Check if already running
    if schedule.status == TriggerStatus.RUNNING:
        return StartResult(False, "already_running")

    # Check outstanding commits
    outstanding = count_outstanding(schedule)
    if outstanding >= schedule.pr_limit:
        update_schedule_status(schedule_id, TriggerStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    # Spawn a single iteration
    update_schedule_status(schedule_id, TriggerStatus.RUNNING)
    subprocess.Popen(
        [sys.executable, "-m", "loopflow.lfd.trigger_runner", "schedule", schedule_id],
        cwd=schedule.repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return StartResult(True)


def _run_loop(loop: Loop) -> None:
    """Run the loop execution until it should pause."""
    from loopflow.lfd.loop_runner import run_loop_iterations

    run_loop_iterations(loop)


def get_wt_from_cwd() -> Path | None:
    """Get the worktree path from current working directory."""
    return find_worktree_root()
