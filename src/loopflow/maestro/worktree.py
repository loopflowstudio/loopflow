"""Agent worktree management.

Each agent runs in an isolated worktree to avoid conflicts with:
- Your active work in the main repo
- Other agents targeting the same repo
"""

import subprocess
from pathlib import Path

from loopflow.maestro.markdown import AgentFile
from loopflow.worktrees import WorktreeError, create, get_path, list_all, remove


def _agent_branch_name(agent: AgentFile) -> str:
    """Get the branch name for an agent's worktree."""
    return f"agent-{agent.name}"


def _sync_to_main(worktree_path: Path) -> bool:
    """Reset worktree to origin/main. Returns success."""
    try:
        subprocess.run(
            ["git", "fetch", "origin", "main"],
            cwd=worktree_path,
            capture_output=True,
            check=True,
        )
        subprocess.run(
            ["git", "reset", "--hard", "origin/main"],
            cwd=worktree_path,
            capture_output=True,
            check=True,
        )
        return True
    except subprocess.CalledProcessError:
        return False


def ensure_agent_worktree(agent: AgentFile) -> Path:
    """Ensure agent has a worktree ready for running.

    - Creates worktree if missing
    - Syncs to origin/main before each run
    - If fresh=True, removes and recreates the worktree

    Returns the worktree path.
    """
    branch_name = _agent_branch_name(agent)
    expected_path = get_path(agent.repo, branch_name)

    # Check if worktree already exists
    try:
        existing = list_all(agent.repo)
        has_worktree = any(wt.branch == branch_name for wt in existing)
    except WorktreeError:
        has_worktree = False

    if agent.fresh and has_worktree:
        # Force fresh start - remove existing worktree
        remove(agent.repo, branch_name)
        has_worktree = False

    if has_worktree:
        # Worktree exists - sync it to main
        if expected_path.exists() and _sync_to_main(expected_path):
            return expected_path

        # Sync failed or path doesn't match - recreate
        remove(agent.repo, branch_name)
        has_worktree = False

    # Create new worktree from main
    worktree_path = create(agent.repo, branch_name, base="main")
    return worktree_path


def remove_agent_worktree(agent: AgentFile) -> bool:
    """Remove an agent's worktree."""
    branch_name = _agent_branch_name(agent)
    return remove(agent.repo, branch_name)


def get_agent_worktree_path(agent: AgentFile) -> Path | None:
    """Get the path to an agent's worktree, if it exists."""
    branch_name = _agent_branch_name(agent)

    try:
        existing = list_all(agent.repo)
        for wt in existing:
            if wt.branch == branch_name:
                return wt.path
    except WorktreeError:
        pass

    return None
