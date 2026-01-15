"""Emoji-based naming for agent worktrees and branches."""

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
