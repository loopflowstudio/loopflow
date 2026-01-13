"""Parse agent markdown files with YAML frontmatter.

Agent files are markdown with YAML frontmatter, like Jekyll or Hugo posts:

```markdown
---
repo: /path/to/repo
pipeline: [implement, polish, land]
trigger: main-changed
context: [src/, docs/]
---

The prompt body goes here. This is what the agent sees.
```
"""

import re
from dataclasses import dataclass
from pathlib import Path


@dataclass
class AgentFile:
    """Parsed agent markdown file."""

    name: str
    path: Path
    repo: Path
    pipeline: list[str]
    trigger: str
    context: list[str]
    prompt: str
    interval_seconds: int | None = None


_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


def parse_agent_file(path: Path) -> AgentFile | None:
    """Parse an agent markdown file. Returns None if invalid."""
    if not path.exists() or not path.suffix == ".md":
        return None

    text = path.read_text()
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return None

    frontmatter = match.group(1)
    prompt = text[match.end():].strip()
    config = _parse_yaml_frontmatter(frontmatter)

    if not config.get("repo") or not config.get("pipeline"):
        return None

    return AgentFile(
        name=path.stem,
        path=path,
        repo=Path(config["repo"]).expanduser(),
        pipeline=config["pipeline"],
        trigger=config.get("trigger", "manual"),
        context=config.get("context", []),
        prompt=prompt,
        interval_seconds=config.get("interval"),
    )


def _parse_yaml_frontmatter(text: str) -> dict:
    """Parse simple YAML frontmatter. Handles basic key: value and lists."""
    result: dict = {}
    current_key = None

    for line in text.split("\n"):
        line = line.rstrip()
        if not line or line.startswith("#"):
            continue

        # Check for list item continuation
        if line.startswith("  - ") and current_key:
            if current_key not in result:
                result[current_key] = []
            result[current_key].append(line[4:].strip())
            continue

        # Parse key: value
        if ":" in line:
            key, _, value = line.partition(":")
            key = key.strip()
            value = value.strip()
            current_key = key

            if not value:
                # Start of a list or empty value
                continue

            # Inline list: [a, b, c]
            if value.startswith("[") and value.endswith("]"):
                items = value[1:-1].split(",")
                result[key] = [item.strip() for item in items if item.strip()]
            # Integer
            elif value.isdigit():
                result[key] = int(value)
            # String value
            else:
                result[key] = value

    return result


def list_agent_files(agents_dir: Path | None = None) -> list[AgentFile]:
    """List all valid agent files from ~/.lf/agents/."""
    if agents_dir is None:
        agents_dir = Path.home() / ".lf" / "agents"

    if not agents_dir.exists():
        return []

    agents = []
    for path in sorted(agents_dir.glob("*.md")):
        agent = parse_agent_file(path)
        if agent:
            agents.append(agent)

    return agents


def get_agent_file(name: str, agents_dir: Path | None = None) -> AgentFile | None:
    """Get a specific agent by name."""
    if agents_dir is None:
        agents_dir = Path.home() / ".lf" / "agents"

    path = agents_dir / f"{name}.md"
    return parse_agent_file(path)


def create_agent_file(
    name: str,
    repo: Path,
    pipeline: list[str],
    trigger: str = "manual",
    context: list[str] | None = None,
    prompt: str = "",
    interval_seconds: int | None = None,
    agents_dir: Path | None = None,
) -> Path:
    """Create a new agent markdown file."""
    if agents_dir is None:
        agents_dir = Path.home() / ".lf" / "agents"

    agents_dir.mkdir(parents=True, exist_ok=True)
    path = agents_dir / f"{name}.md"

    lines = ["---"]
    lines.append(f"repo: {repo}")
    lines.append(f"pipeline: [{', '.join(pipeline)}]")
    lines.append(f"trigger: {trigger}")
    if interval_seconds and trigger == "interval":
        lines.append(f"interval: {interval_seconds}")
    if context:
        lines.append(f"context: [{', '.join(context)}]")
    lines.append("---")
    lines.append("")
    lines.append(prompt or "Describe what this agent should do.")

    path.write_text("\n".join(lines) + "\n")
    return path


def delete_agent_file(name: str, agents_dir: Path | None = None) -> bool:
    """Delete an agent markdown file."""
    if agents_dir is None:
        agents_dir = Path.home() / ".lf" / "agents"

    path = agents_dir / f"{name}.md"
    if path.exists():
        path.unlink()
        return True
    return False
