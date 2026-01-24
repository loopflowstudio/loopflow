"""Goal file loading for agent judgment and intent."""

import re
from dataclasses import dataclass
from pathlib import Path

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)

# Path to bundled builtin goal templates
_GOALS_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "goals"


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


@dataclass
class Goal:
    """A parsed goal file.

    Goals define *how* to approach work (judgment, style, intent).
    They don't specify *where* (area) or *what* (flow) - those are separate dimensions.
    """

    name: str
    content: str


def _get_builtin_goal(name: str) -> Path | None:
    """Return path to bundled goal template if it exists."""
    builtin = _GOALS_TEMPLATES_DIR / f"{name}.md"
    return builtin if builtin.exists() else None


def list_builtin_goals() -> list[str]:
    """Return names of all builtin goals."""
    if not _GOALS_TEMPLATES_DIR.exists():
        return []
    return sorted(p.stem for p in _GOALS_TEMPLATES_DIR.glob("*.md"))


def load_goal(repo: Path | None, goal_name: str) -> Goal | None:
    """Load and parse a goal file.

    Checks in order:
    1. .lf/goals/{name}.md (repo)
    2. ~/.lf/goals/{name}.md (global)
    3. templates/goals/{name}.md (builtin)

    Returns None if goal file doesn't exist.
    """
    if not goal_name:
        return None

    # Check repo goal first
    goal_path = None
    if repo:
        repo_goal = repo / ".lf" / "goals" / f"{goal_name}.md"
        if repo_goal.exists():
            goal_path = repo_goal

    # Check global goal
    if not goal_path:
        global_goal = Path.home() / ".lf" / "goals" / f"{goal_name}.md"
        if global_goal.exists():
            goal_path = global_goal

    # Fall back to builtin templates
    if not goal_path:
        builtin_path = _get_builtin_goal(goal_name)
        if builtin_path:
            goal_path = builtin_path
        else:
            return None

    text = goal_path.read_text()
    _frontmatter, content = _parse_frontmatter(text)

    return Goal(name=goal_name, content=content)


def load_goal_content(repo: Path, goal_name: str) -> str | None:
    """Load just the goal file content."""
    goal = load_goal(repo, goal_name)
    return goal.content if goal else None


def list_goals(repo: Path | None) -> list[str]:
    """List available goal names (repo, global, and builtin)."""
    goals = set()

    # Repo goals
    if repo:
        repo_goals_dir = repo / ".lf" / "goals"
        if repo_goals_dir.exists():
            goals.update(p.stem for p in repo_goals_dir.glob("*.md"))

    # Global goals
    global_goals_dir = Path.home() / ".lf" / "goals"
    if global_goals_dir.exists():
        goals.update(p.stem for p in global_goals_dir.glob("*.md"))

    # Builtin goals
    goals.update(list_builtin_goals())

    return sorted(goals)


def goal_exists(repo: Path | None, goal_name: str) -> bool:
    """Check if a goal file exists (repo, global, or builtin)."""
    if not goal_name:
        return False
    # Check repo goal
    if repo:
        repo_goal = repo / ".lf" / "goals" / f"{goal_name}.md"
        if repo_goal.exists():
            return True
    # Check global goal
    global_goal = Path.home() / ".lf" / "goals" / f"{goal_name}.md"
    if global_goal.exists():
        return True
    # Check builtin goal
    return _get_builtin_goal(goal_name) is not None


def resolve_goals(repo: Path, goal_names: list[str]) -> list[Goal]:
    """Load and resolve goal names to Goal objects."""
    goals = []
    for name in goal_names:
        goal = load_goal(repo, name)
        if goal:
            goals.append(goal)
    return goals


def parse_goal_arg(goal_arg: str | None) -> list[str]:
    """Parse 'a,b,c' into ['a', 'b', 'c']. Returns [] if None or empty."""
    if not goal_arg:
        return []
    return [g.strip() for g in goal_arg.split(",") if g.strip()]


def format_goal_section(goal_names: list[str] | None, repo_root: Path) -> str | None:
    """Load goals and format as XML section for prompt."""
    if not goal_names:
        return None

    loaded = []
    for name in goal_names:
        goal = load_goal(repo_root, name)
        if goal:
            loaded.append(goal)

    if not loaded:
        return None

    parts = [f"<lf:goal:{goal.name}>\n{goal.content}\n</lf:goal:{goal.name}>" for goal in loaded]

    if len(parts) == 1:
        return parts[0]
    return "<lf:goals>\n" + "\n\n".join(parts) + "\n</lf:goals>"
