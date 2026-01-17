"""Emoji-based naming for agent worktrees and branches."""

import shutil
import subprocess
from pathlib import Path

from loopflow.lfd.models import AgentSpec


def agent_branch_name(agent: AgentSpec, iteration: int) -> str:
    """Generate branch name: {emoji}/{agent}/{iteration:03d}."""
    if agent.emoji:
        return f"{agent.emoji}/{agent.name}/{iteration:03d}"
    return f"agent/{agent.name}/{iteration:03d}"


def agent_worktree_name(repo_name: str, agent: AgentSpec, iteration: int) -> str:
    """Generate worktree directory name: {repo}.{emoji}-{agent}-{iteration:03d}."""
    if agent.emoji:
        return f"{repo_name}.{agent.emoji}-{agent.name}-{iteration:03d}"
    return f"{repo_name}.agent-{agent.name}-{iteration:03d}"


def agent_worktree_path(repo: Path, agent: AgentSpec, iteration: int) -> Path:
    """Generate worktree path as sibling to repo."""
    repo_name = repo.name
    worktree_name = agent_worktree_name(repo_name, agent, iteration)
    return repo.parent / worktree_name


def agent_pr_title(agent: AgentSpec, summary: str) -> str:
    """Generate PR title with emoji prefix: {emoji} {summary}."""
    if agent.emoji:
        return f"{agent.emoji} {summary}"
    return summary


def parse_agent_branch(branch: str) -> tuple[str, str, int] | None:
    """Parse branch name to extract (emoji, agent_name, iteration).

    Returns None if branch doesn't match agent pattern.
    """
    parts = branch.split("/")
    if len(parts) != 3:
        return None

    prefix, name, iteration_str = parts

    # Check if it's an agent branch
    if prefix == "agent":
        emoji = ""
    else:
        # First part should be an emoji
        emoji = prefix

    try:
        iteration = int(iteration_str)
    except ValueError:
        return None

    return emoji, name, iteration


def get_current_branch(worktree: Path) -> str | None:
    """Get the current branch name in a worktree."""
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip() or None


def rename_branch(worktree: Path, new_name: str) -> bool:
    """Rename current branch and update worktree directory to match.

    Returns True on success.
    """
    current = get_current_branch(worktree)
    if not current:
        return False

    # Rename the branch
    result = subprocess.run(
        ["git", "branch", "-m", new_name],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    # Calculate new worktree directory name
    # Pattern: {repo}.{branch} where branch slashes become dashes
    repo_prefix = worktree.name.split(".")[0]  # Get "repo" from "repo.branch-name"
    new_dir_name = f"{repo_prefix}.{new_name.replace('/', '-')}"
    new_path = worktree.parent / new_dir_name

    if new_path == worktree:
        return True  # No directory rename needed

    if new_path.exists():
        return True  # Can't rename, but branch rename succeeded

    # Move the worktree directory
    result = subprocess.run(
        ["git", "worktree", "move", str(worktree), str(new_path)],
        cwd=worktree.parent,
        capture_output=True,
        text=True,
    )
    # Even if move fails, branch was renamed successfully
    return True


def iteration_worktree_path(repo: Path, agent_name: str, temp_branch: str) -> Path:
    """Generate worktree path for an iteration branch.

    Pattern: {repo}.{branch} as sibling to main repo.
    """
    repo_name = repo.name
    dir_name = f"{repo_name}.{temp_branch}"
    return repo.parent / dir_name


def personal_main_worktree_path(repo: Path, personal_main: str) -> Path:
    """Generate worktree path for personal-main branch.

    Pattern: {repo}.{personal_main} as sibling to main repo.
    """
    repo_name = repo.name
    return repo.parent / f"{repo_name}.{personal_main}"
