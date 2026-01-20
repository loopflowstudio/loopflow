"""Goal file loading for agent loops."""

from pathlib import Path


def load_goal(repo: Path, goal_name: str) -> str | None:
    """Load goal file content from .lf/goals/{name}.md.

    Returns None if goal file doesn't exist.
    """
    if not goal_name:
        return None

    goal_path = repo / ".lf" / "goals" / f"{goal_name}.md"
    if not goal_path.exists():
        return None

    return goal_path.read_text()


def list_goals(repo: Path) -> list[str]:
    """List available goal names in a repo."""
    goals_dir = repo / ".lf" / "goals"
    if not goals_dir.exists():
        return []

    return [p.stem for p in sorted(goals_dir.glob("*.md"))]


def goal_exists(repo: Path, goal_name: str) -> bool:
    """Check if a goal file exists."""
    if not goal_name:
        return False
    goal_path = repo / ".lf" / "goals" / f"{goal_name}.md"
    return goal_path.exists()
