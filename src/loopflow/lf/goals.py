"""Goal file loading for agent loops."""

import re
from dataclasses import dataclass
from pathlib import Path

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


@dataclass
class Goal:
    """A parsed goal file."""
    name: str
    content: str
    area: list[str]      # Default pathset
    pipeline: str        # Default pipeline


def load_goal(repo: Path, goal_name: str) -> Goal | None:
    """Load and parse a goal file from .lf/goals/{name}.md.

    Returns None if goal file doesn't exist.
    """
    if not goal_name:
        return None

    goal_path = repo / ".lf" / "goals" / f"{goal_name}.md"
    if not goal_path.exists():
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


def _parse_frontmatter(text: str) -> tuple[dict, str]:
    """Parse YAML frontmatter from markdown text.

    Returns (frontmatter_dict, body_content).
    """
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return {}, text

    frontmatter_text = match.group(1)
    body = text[match.end():].strip()

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
