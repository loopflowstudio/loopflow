"""Parse YAML frontmatter from task files."""

import re
from dataclasses import dataclass, field
from typing import Any

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


@dataclass
class TaskConfig:
    """Per-task configuration from frontmatter."""

    interactive: bool | None = None
    include: list[str] | None = None
    exclude: list[str] | None = None
    model: str | None = None
    voice: list[str] | None = None


@dataclass
class TaskFile:
    """Parsed task file with frontmatter and content."""

    name: str
    content: str
    config: TaskConfig = field(default_factory=TaskConfig)


@dataclass
class ResolvedTaskConfig:
    """Fully resolved task configuration after merging all sources."""

    interactive: bool
    include: list[str]
    exclude: list[str]
    model: str
    context: list[str]
    voice: list[str]


def parse_task_file(name: str, text: str) -> TaskFile:
    """Parse a task file, extracting frontmatter if present."""
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return TaskFile(name=name, content=text, config=TaskConfig())

    frontmatter = match.group(1)
    content = text[match.end() :]
    config_dict = _parse_yaml_frontmatter(frontmatter)

    # Normalize voice: can be string or list
    voice_raw = config_dict.get("voice")
    if isinstance(voice_raw, str):
        voice = [voice_raw] if voice_raw else None
    else:
        voice = voice_raw if voice_raw else None

    return TaskFile(
        name=name,
        content=content,
        config=TaskConfig(
            interactive=config_dict.get("interactive"),
            include=config_dict.get("include"),
            exclude=config_dict.get("exclude"),
            model=config_dict.get("model"),
            voice=voice,
        ),
    )


def _parse_yaml_frontmatter(text: str) -> dict[str, Any]:
    """Parse simple YAML frontmatter.

    Handles:
    - key: value pairs
    - key: [inline, list] syntax
    - key:\\n  - list\\n  - items syntax
    - boolean values (true/false, yes/no)
    - integers
    """
    result: dict[str, Any] = {}
    lines = text.strip().split("\n")
    i = 0

    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.strip().startswith("#"):
            i += 1
            continue

        if ":" not in line:
            i += 1
            continue

        key, _, value = line.partition(":")
        key = key.strip()
        value = value.strip()

        # Handle inline list: key: [a, b, c]
        if value.startswith("[") and value.endswith("]"):
            items = value[1:-1].split(",")
            result[key] = [item.strip().strip("\"'") for item in items if item.strip()]
            i += 1
            continue

        # Handle multi-line list
        if not value:
            items = []
            i += 1
            while i < len(lines) and lines[i].startswith("  - "):
                item = lines[i].strip()[2:].strip().strip("\"'")
                items.append(item)
                i += 1
            if items:
                result[key] = items
            continue

        # Handle scalar values
        result[key] = _parse_scalar(value)
        i += 1

    return result


def _parse_scalar(value: str) -> Any:
    """Parse a scalar YAML value."""
    value = value.strip().strip("\"'")

    # Booleans
    if value.lower() in ("true", "yes"):
        return True
    if value.lower() in ("false", "no"):
        return False

    # Integers
    try:
        return int(value)
    except ValueError:
        pass

    return value


def get_defaults() -> TaskConfig:
    """Return default task configuration."""
    return TaskConfig(
        interactive=False,
        include=None,
        exclude=["tests/**"],
        model=None,
    )


def resolve_task_config(
    task_name: str,
    global_config,  # Config | None - avoid circular import
    frontmatter: TaskConfig,
    cli_interactive: bool | None,
    cli_auto: bool | None,
    cli_model: str | None,
    cli_context: list[str] | None,
    cli_voice: list[str] | None = None,
) -> ResolvedTaskConfig:
    """Merge configs: CLI > frontmatter > global > defaults."""
    defaults = get_defaults()

    # Resolve interactive: CLI > frontmatter > global (interactive list) > default
    if cli_interactive:
        interactive = True
    elif cli_auto:
        interactive = False
    elif frontmatter.interactive is not None:
        interactive = frontmatter.interactive
    elif global_config and task_name in global_config.interactive:
        interactive = True
    else:
        interactive = defaults.interactive or False

    # Resolve model: CLI > frontmatter > global > default
    if cli_model:
        model = cli_model
    elif frontmatter.model:
        model = frontmatter.model
    elif global_config:
        model = global_config.agent_model
    else:
        model = "claude:opus"

    # Resolve exclude: frontmatter > global > default
    if frontmatter.exclude is not None:
        exclude = list(frontmatter.exclude)
    elif global_config and global_config.exclude:
        exclude = list(global_config.exclude)
    else:
        exclude = list(defaults.exclude) if defaults.exclude else []

    # Resolve include: frontmatter > legacy include_tests_for > default
    if frontmatter.include is not None:
        include = list(frontmatter.include)
    elif global_config and global_config.include_tests_for:
        # Legacy: include_tests_for: [polish, implement] means those tasks include tests
        if task_name in global_config.include_tests_for:
            include = ["tests/**"]
        else:
            include = []
    else:
        include = []

    # If include contains tests/**, remove from exclude
    if "tests/**" in include and "tests/**" in exclude:
        exclude.remove("tests/**")

    # Resolve context: CLI extends frontmatter extends global
    context: list[str] = []
    if global_config and global_config.context:
        context.extend(global_config.context)
    if cli_context:
        context.extend(cli_context)

    # Resolve voice: CLI > frontmatter > global > none
    if cli_voice:
        voice = list(cli_voice)
    elif frontmatter.voice:
        voice = list(frontmatter.voice)
    elif global_config and global_config.voice:
        voice = list(global_config.voice)
    else:
        voice = []

    return ResolvedTaskConfig(
        interactive=interactive,
        include=include,
        exclude=exclude,
        model=model,
        context=context,
        voice=voice,
    )
