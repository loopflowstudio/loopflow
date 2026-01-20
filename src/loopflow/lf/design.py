"""Design artifact helpers."""

import shutil
from pathlib import Path


def gather_design_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Gather design docs from .design/ for prompt context."""
    design_dir = repo_root / ".design"
    if not design_dir.is_dir():
        return []

    docs = []
    for path in sorted(design_dir.rglob("*.md")):
        if path.is_file():
            docs.append((path, path.read_text()))
    return docs


def gather_internal_docs(repo_root: Path) -> list[tuple[Path, str]]:
    """Gather internal docs from .docs/ for prompt context.

    .docs/ contains forward-looking internal documentation:
    architecture, decisions, context for agents. Unlike .design/
    (ephemeral per-PR), .docs/ persists across merges.
    """
    docs_dir = repo_root / ".docs"
    if not docs_dir.is_dir():
        return []

    docs = []
    for path in sorted(docs_dir.rglob("*.md")):
        if path.is_file():
            docs.append((path, path.read_text()))
    return docs


def load_goal(goal: str | Path, repo_root: Path) -> str | None:
    """Load goal content from .lf/goals/{name}.md or a direct path.

    Goals are high-level directives for autonomous agent loops.
    Unlike tasks (single-purpose prompts), goals describe ongoing
    objectives an agent works toward across multiple iterations.

    Args:
        goal: Goal name (e.g., "test-coverage") or path (e.g., ".lf/goals/test-coverage.md")
        repo_root: Repository root path

    Returns:
        Goal content as string, or None if not found.
    """
    goal_str = str(goal)

    # If it's just a name (no path separator), look in .lf/goals/
    if "/" not in goal_str and "\\" not in goal_str:
        goal_path = repo_root / ".lf" / "goals" / f"{goal_str}.md"
    else:
        # It's a path, resolve relative to repo root
        goal_path = repo_root / goal_str

    if goal_path.exists() and goal_path.is_file():
        return goal_path.read_text()
    return None


def has_design_artifacts(repo_root: Path) -> bool:
    """Return True when .design contains any files or folders."""
    design_dir = repo_root / ".design"
    if not design_dir.exists():
        return False
    return any(design_dir.iterdir())


def clear_design_artifacts(repo_root: Path) -> bool:
    """Remove .design contents while keeping the folder."""
    design_dir = repo_root / ".design"
    if design_dir.exists() and not design_dir.is_dir():
        design_dir.unlink()
        design_dir.mkdir(exist_ok=True)
        return True

    if not design_dir.exists():
        return False

    removed = False
    for path in list(design_dir.iterdir()):
        removed = True
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()

    design_dir.mkdir(exist_ok=True)
    return removed
