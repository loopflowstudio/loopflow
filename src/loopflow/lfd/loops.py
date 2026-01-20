"""Loop management for lfd."""

import os
import signal
import subprocess
import uuid
from pathlib import Path

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


def _allocate_personal_main(repo: Path, goal_name: str) -> str:
    """Return available branch name: goal-main, goal-1-main, etc."""
    candidate = f"{goal_name}-main"
    if not _branch_exists(repo, candidate):
        return candidate
    for i in range(1, 100):
        candidate = f"{goal_name}-{i}-main"
        if not _branch_exists(repo, candidate):
            return candidate
    raise ValueError(f"Could not allocate personal-main branch for {goal_name}")


def _create_personal_main_branch(repo: Path, branch: str) -> None:
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
    """Create or get an existing loop for a goal+repo combination."""
    # Check if loop already exists
    existing = get_loop_by_goal_repo(loop_type, goal_name, repo)
    if existing:
        return existing

    # Allocate and create personal-main branch
    personal_main = _allocate_personal_main(repo, goal_name)
    _create_personal_main_branch(repo, personal_main)

    loop = Loop(
        id=str(uuid.uuid4()),
        type=loop_type,
        goal=goal_name,
        repo=repo,
        personal_main=personal_main,
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
        ["git", "fetch", "origin", "main", loop.personal_main],
        cwd=loop.repo,
        capture_output=True,
    )

    # Count commits ahead
    result = subprocess.run(
        ["git", "rev-list", "--count", f"origin/main..origin/{loop.personal_main}"],
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


def start_loop(loop_id: str, foreground: bool = False) -> bool:
    """Mark a loop as running and start execution.

    If foreground=True, runs the loop in the current process.
    Otherwise spawns a background subprocess.
    """
    loop = get_loop(loop_id)
    if not loop:
        return False

    # Check if already running
    if loop.status == LoopStatus.RUNNING and loop.pid and is_process_running(loop.pid):
        return False

    # Check outstanding commits before starting
    outstanding = count_outstanding(loop)
    if outstanding >= loop.pr_limit:
        update_loop_status(loop_id, LoopStatus.WAITING)
        return False

    if foreground:
        # Run directly in this process
        update_loop_status(loop_id, LoopStatus.RUNNING)
        update_loop_pid(loop_id, os.getpid())
        _run_loop(loop)
        return True
    else:
        # Spawn background process
        import sys
        proc = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfd.loop_runner", loop_id],
            cwd=loop.repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        update_loop_status(loop_id, LoopStatus.RUNNING)
        update_loop_pid(loop_id, proc.pid)
        return True


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
