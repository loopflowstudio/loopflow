"""Direction file loading for wave judgment and intent."""

import re
from dataclasses import dataclass
from pathlib import Path

from loopflow.lf.files import find_md_in_dir, list_md_grouped

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)

# Path to bundled builtin directions
_BUILTINS_DIRECTIONS_DIR = Path(__file__).parent / "builtins" / "directions"


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
class Direction:
    """A parsed direction file.

    Directions define *how* to approach work (judgment, style, intent).
    They don't specify *where* (area) or *what* (flow) - those are separate dimensions.
    """

    name: str
    content: str


def _get_builtin_direction(name: str) -> Path | None:
    """Return path to bundled direction if it exists."""
    return find_md_in_dir(_BUILTINS_DIRECTIONS_DIR, name)


def list_builtin_directions() -> list[str]:
    """Return names of all builtin directions."""
    grouped = list_md_grouped(_BUILTINS_DIRECTIONS_DIR)
    return sorted(name for names in grouped.values() for name in names)


def list_builtin_directions_grouped() -> dict[str, list[str]]:
    """Return builtin directions grouped by folder."""
    return list_md_grouped(_BUILTINS_DIRECTIONS_DIR)


def load_direction(repo: Path | None, direction_name: str) -> Direction | None:
    """Load and parse a direction file.

    Checks in order:
    1. .lf/directions/{name}.md (repo, including subdirs)
    2. ~/.lf/directions/{name}.md (global, including subdirs)
    3. builtins/directions/{name}.md (builtin, including subdirs)

    Supports both 'name' and 'folder/name' formats.
    Returns None if direction file doesn't exist.
    """
    if not direction_name:
        return None

    # Check repo direction first
    direction_path = None
    if repo:
        direction_path = find_md_in_dir(repo / ".lf" / "directions", direction_name)

    # Check global direction
    if not direction_path:
        direction_path = find_md_in_dir(Path.home() / ".lf" / "directions", direction_name)

    # Fall back to builtins
    if not direction_path:
        direction_path = _get_builtin_direction(direction_name)

    if not direction_path:
        return None

    text = direction_path.read_text()
    _frontmatter, content = _parse_frontmatter(text)

    return Direction(name=direction_name, content=content)


def load_direction_content(repo: Path, direction_name: str) -> str | None:
    """Load just the direction file content."""
    direction = load_direction(repo, direction_name)
    return direction.content if direction else None


def list_directions(repo: Path | None) -> list[str]:
    """List available direction names (repo, global, and builtin)."""
    grouped = list_directions_grouped(repo)
    return sorted(name for names in grouped.values() for name in names)


def list_directions_grouped(repo: Path | None) -> dict[str, list[str]]:
    """List available directions grouped by folder."""
    grouped: dict[str, list[str]] = {}

    def merge(source: dict[str, list[str]], seen: set[str]) -> None:
        for folder, names in source.items():
            for name in names:
                if name not in seen:
                    seen.add(name)
                    grouped.setdefault(folder, []).append(name)

    seen: set[str] = set()

    # Repo directions (highest priority)
    if repo:
        merge(list_md_grouped(repo / ".lf" / "directions"), seen)

    # Global directions
    merge(list_md_grouped(Path.home() / ".lf" / "directions"), seen)

    # Builtin directions
    merge(list_builtin_directions_grouped(), seen)

    # Sort within each group
    for folder in grouped:
        grouped[folder] = sorted(grouped[folder])

    return grouped


def resolve_directions(repo: Path, direction_names: list[str]) -> list[Direction]:
    """Load and resolve direction names to Direction objects."""
    directions = []
    for name in direction_names:
        direction = load_direction(repo, name)
        if direction:
            directions.append(direction)
    return directions


def parse_list_arg(arg: str | list[str] | None) -> list[str]:
    """Parse CLI arg into list. Accepts 'a,b,c', ['a', 'b'], or None.

    Works for any multi-value option: direction, area, flow, etc.
    Handles both repeated flags (-d a -d b) and comma-separated (-d a,b).
    """
    if not arg:
        return []
    if isinstance(arg, str):
        return [item.strip() for item in arg.split(",") if item.strip()]
    # List input - expand any comma-separated items within
    result = []
    for item in arg:
        result.extend(s.strip() for s in item.split(",") if s.strip())
    return result


# Alias for backwards compatibility
parse_direction_arg = parse_list_arg


def format_direction_section(direction_names: list[str] | None, repo_root: Path) -> str | None:
    """Load directions and format as XML section for prompt."""
    if not direction_names:
        return None

    loaded = []
    for name in direction_names:
        direction = load_direction(repo_root, name)
        if direction:
            loaded.append(direction)

    if not loaded:
        return None

    parts = [f"<lf:direction:{d.name}>\n{d.content}\n</lf:direction:{d.name}>" for d in loaded]

    if len(parts) == 1:
        return parts[0]
    return "<lf:directions>\n" + "\n\n".join(parts) + "\n</lf:directions>"
