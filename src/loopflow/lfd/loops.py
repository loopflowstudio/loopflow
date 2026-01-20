"""Loop management for lfd."""

import os
import random
import signal
import subprocess
import sys
import uuid
from pathlib import Path


# Word lists for generating unique branch names (matches Maestro/NameGenerator.swift)
MAGICAL = [
    "aurora", "cascade", "crystal", "drift", "echo", "ember",
    "fern", "flume", "frost", "glade", "grove", "haze",
    "ivy", "jade", "luna", "mist", "nova", "opal",
    "petal", "prism", "rain", "ripple", "sage", "shade",
    "spark", "star", "stone", "storm", "tide", "vale",
    "wave", "wisp", "wren", "zephyr",
]

MUSICAL = [
    "allegro", "aria", "ballad", "cadence", "canon", "chord",
    "coda", "duet", "forte", "fugue", "harmony", "hymn",
    "lilt", "lyric", "melody", "motif", "opus", "prelude",
    "refrain", "rondo", "sonata", "tempo", "trill", "tune",
    "verse", "waltz",
]


def _generate_random_words() -> str:
    """Generate a random magical-musical pair like 'aurora-melody'."""
    magical = random.choice(MAGICAL)
    musical = random.choice(MUSICAL)
    return f"{magical}-{musical}"

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo
from loopflow.lfd.db import (
    get_loop,
    get_loop_by_goal_repo,
    save_loop,
    update_loop_pid,
    update_loop_status,
)
from loopflow.lfd.models import Loop, LoopStatus, LoopType, MergeMode
from loopflow.lfd.process import is_process_running


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


def _allocate_loop_main(repo: Path, goal_name: str, area: str | None = None) -> str:
    """Return unique branch name: goal-area-words-main or goal-words-main.

    Always includes random words for uniqueness. Format:
    - With area: product-engineer-api-swift-river-main
    - Without area: product-engineer-swift-river-main
    """
    if area:
        # "Maestro/" -> "maestro", "src/loopflow/" -> "loopflow"
        area_slug = area.rstrip("/").split("/")[-1].lower()
        base = f"{goal_name}-{area_slug}"
    else:
        base = goal_name

    # Try random word combinations until we find an available branch
    for _ in range(100):
        words = _generate_random_words()
        candidate = f"{base}-{words}-main"
        if not _branch_exists(repo, candidate):
            return candidate

    raise ValueError(f"Could not allocate personal-main branch for {base}")


def _create_loop_main_branch(repo: Path, branch: str) -> None:
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


def create_loop(
    loop_type: LoopType,
    goal_name: str,
    repo: Path,
    area: str | None = None,
    project_file: str | None = None,
    pathset: str | None = None,
    cron: str | None = None,
) -> Loop:
    """Create or get an existing loop for a goal+area+repo combination."""
    # Check if loop already exists (now includes area in lookup)
    existing = get_loop_by_goal_repo(loop_type, goal_name, repo, area=area)
    if existing:
        return existing

    # Allocate and create personal-main branch (now includes area in name)
    loop_main = _allocate_loop_main(repo, goal_name, area=area)
    _create_loop_main_branch(repo, loop_main)

    loop = Loop(
        id=str(uuid.uuid4()),
        type=loop_type,
        goal_name=goal_name,
        repo=repo,
        loop_main=loop_main,
        status=LoopStatus.IDLE,
        area=area,
        project_file=project_file,
        pathset=pathset,
        cron=cron,
    )

    save_loop(loop)
    return loop


def count_outstanding(loop: Loop) -> int:
    """Count commits on personal-main ahead of main.

    Returns number of commits on personal-main that are not yet on main.
    """
    # Ensure we have latest refs
    subprocess.run(
        ["git", "fetch", "origin", "main", loop.loop_main],
        cwd=loop.repo,
        capture_output=True,
    )

    # Count commits ahead
    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{loop.loop_main}"],
        cwd=loop.repo,
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
    """Result of attempting to start a loop."""

    def __init__(self, ok: bool, reason: str | None = None, outstanding: int | None = None):
        self.ok = ok
        self.reason = reason
        self.outstanding = outstanding

    def __bool__(self) -> bool:
        return self.ok


def start_loop(loop_id: str, foreground: bool = False) -> StartResult:
    """Mark a loop as running and start execution.

    If foreground=True, runs the loop in the current process.
    Otherwise spawns a background subprocess.

    Returns a StartResult indicating success or failure reason.
    """
    loop = get_loop(loop_id)
    if not loop:
        return StartResult(False, "not_found")

    # Check if already running
    if loop.status == LoopStatus.RUNNING and loop.pid and is_process_running(loop.pid):
        return StartResult(False, "already_running")

    # Check outstanding commits before starting
    outstanding = count_outstanding(loop)
    if outstanding >= loop.pr_limit:
        update_loop_status(loop_id, LoopStatus.WAITING)
        return StartResult(False, "waiting", outstanding=outstanding)

    if foreground:
        # Run directly in this process
        update_loop_status(loop_id, LoopStatus.RUNNING)
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
        update_loop_status(loop_id, LoopStatus.RUNNING)
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

    update_loop_status(loop_id, LoopStatus.IDLE)
    update_loop_pid(loop_id, None)
    return True


def _run_loop(loop: Loop) -> None:
    """Run the loop execution until it should pause."""
    from loopflow.lfd.loop_runner import run_loop_iterations
    run_loop_iterations(loop)


def get_repo_from_cwd() -> Path | None:
    """Get the main repo path from current working directory."""
    worktree_root = find_worktree_root()
    if not worktree_root:
        return None
    return find_main_repo(worktree_root) or worktree_root
