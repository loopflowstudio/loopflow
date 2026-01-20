"""Goal file loading for agent loops."""

import re
from dataclasses import dataclass
from pathlib import Path

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)

# Path to bundled builtin goal templates
_GOALS_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "goals"


@dataclass
class Goal:
    """A parsed goal file."""

    name: str
    content: str
    area: list[str]  # Default pathset
    pipeline: str  # Default pipeline


def _get_builtin_goal(name: str) -> Path | None:
    """Return path to bundled goal template if it exists."""
    builtin = _GOALS_TEMPLATES_DIR / f"{name}.md"
    return builtin if builtin.exists() else None


def list_builtin_goals() -> list[str]:
    """Return names of all builtin goals."""
    if not _GOALS_TEMPLATES_DIR.exists():
        return []
    return sorted(p.stem for p in _GOALS_TEMPLATES_DIR.glob("*.md"))


def load_goal(repo: Path, goal_name: str) -> Goal | None:
    """Load and parse a goal file.

    Checks in order:
    1. .lf/goals/{name}.md (user-defined)
    2. templates/goals/{name}.md (builtin fallback)

    Returns None if goal file doesn't exist.
    """
    if not goal_name:
        return None

    # Check user-defined goal first
    goal_path = repo / ".lf" / "goals" / f"{goal_name}.md"
    if not goal_path.exists():
        # Fall back to builtin templates
        builtin_path = _get_builtin_goal(goal_name)
        if builtin_path:
            goal_path = builtin_path
        else:
            return None

    text = goal_path.read_text()
    frontmatter, content = _parse_frontmatter(text)

    # Parse area as list
    area = frontmatter.get("area", [])
    if isinstance(area, str):
        area = [a.strip() for a in area.split(",") if a.strip()]

    return Goal(
        name=goal_name,
        content=content,
        area=area,
        pipeline=frontmatter.get("pipeline", "@ship"),
    )


def load_goal_content(repo: Path, goal_name: str) -> str | None:
    """Load just the goal file content (for backwards compatibility)."""
    goal = load_goal(repo, goal_name)
    return goal.content if goal else None


def list_goals(repo: Path) -> list[str]:
    """List available goal names in a repo (including builtins)."""
    goals = set()

    # User-defined goals
    goals_dir = repo / ".lf" / "goals"
    if goals_dir.exists():
        goals.update(p.stem for p in goals_dir.glob("*.md"))

    # Builtin goals
    goals.update(list_builtin_goals())

    return sorted(goals)


def goal_exists(repo: Path, goal_name: str) -> bool:
    """Check if a goal file exists (user-defined or builtin)."""
    if not goal_name:
        return False
    # Check user-defined goal
    goal_path = repo / ".lf" / "goals" / f"{goal_name}.md"
    if goal_path.exists():
        return True
    # Check builtin goal
    return _get_builtin_goal(goal_name) is not None


def _parse_frontmatter(text: str) -> tuple[dict, str]:
    """Parse YAML frontmatter from markdown text.

    Returns (frontmatter_dict, body_content).
    """
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return {}, text

    frontmatter_text = match.group(1)
    body = text[match.end() :].strip()

    # Simple YAML parsing (no external dependency)
    result: dict = {}
    current_key = None

    for line in frontmatter_text.split("\n"):
        line = line.rstrip()
        if not line or line.startswith("#"):
            continue

        # List item continuation
        if line.startswith("  - ") and current_key:
            if current_key not in result:
                result[current_key] = []
            result[current_key].append(line[4:].strip())
            continue

        if ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            value = value.strip()
            current_key = key

            if not value:
                continue

            # Inline list: [a, b, c]
            if value.startswith("[") and value.endswith("]"):
                items = value[1:-1].split(",")
                result[key] = [item.strip() for item in items if item.strip()]
            elif value.lower() in ("true", "yes"):
                result[key] = True
            elif value.lower() in ("false", "no"):
                result[key] = False
            elif value.isdigit():
                result[key] = int(value)
            else:
                result[key] = value

    return result, body
