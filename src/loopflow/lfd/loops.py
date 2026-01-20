"""Loop management for lfd."""

import subprocess
import uuid
from pathlib import Path

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo
from loopflow.lf.goals import Goal, goal_exists, list_goals, load_goal
from loopflow.lfd.db import (
    delete_loop,
    get_loop,
    get_loop_by_goal_repo,
    get_loop_runs,
    list_loops,
    save_loop,
    update_loop_status,
)
from loopflow.lfd.models import Loop, LoopStatus, LoopType, MergeMode


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


def start_loop(loop_id: str) -> bool:
    """Mark a loop as running and start execution."""
    loop = get_loop(loop_id)
    if not loop:
        return False

    # TODO: Actually spawn the subprocess to run the loop
    # For now, just mark it as running
    update_loop_status(loop_id, LoopStatus.RUNNING)
    return True


def stop_loop(loop_id: str) -> bool:
    """Stop a running loop."""
    loop = get_loop(loop_id)
    if not loop:
        return False

    # TODO: Actually kill the subprocess if running
    update_loop_status(loop_id, LoopStatus.IDLE)
    return True


def get_repo_from_cwd() -> Path | None:
    """Get the main repo path from current working directory."""
    worktree_root = find_worktree_root()
    if not worktree_root:
        return None
    return find_main_repo(worktree_root) or worktree_root
