"""Parse YAML frontmatter from step files."""

import re
from dataclasses import dataclass, field
from typing import Any

from pydantic import BaseModel, ConfigDict, field_validator

_FRONTMATTER_PATTERN = re.compile(r"^---\s*\n(.*?)\n---\s*\n?", re.DOTALL)


class StepConfig(BaseModel):
    """Per-step configuration from frontmatter or pipeline overrides."""

    model_config = ConfigDict(extra="ignore")

    interactive: bool | None = None
    include: list[str] | None = None
    exclude: list[str] | None = None
    model: str | None = None
    direction: list[str] | None = None
    chrome: bool | None = None  # Enable Chrome integration for Claude Code
    diff_files: bool | None = None  # Include files changed on branch
    context: list[str] | None = None  # Additional context files (from flow overrides)
    area: str | None = None  # Area path for parent doc inclusion (e.g., "lf/cli")

    @field_validator("direction", mode="before")
    @classmethod
    def _normalize_direction(cls, value):
        if value is None or value == "":
            return None
        if isinstance(value, str):
            return [value]
        return value

    @field_validator("include", "exclude", "context", mode="before")
    @classmethod
    def _normalize_lists(cls, value):
        if value is None:
            return None
        if isinstance(value, str):
            return [item for item in value.split() if item]
        return value

    def to_dict(self) -> dict:
        return self.model_dump(exclude_none=True)

    @classmethod
    def from_dict(cls, data: dict) -> "StepConfig":
        return cls.model_validate(data)


@dataclass
class StepFile:
    """Parsed step file with frontmatter and content."""

    name: str
    content: str
    config: StepConfig = field(default_factory=StepConfig)
    is_external_skill: bool = False


@dataclass
class ResolvedStepConfig:
    """Fully resolved step configuration after merging all sources."""

    interactive: bool
    include: list[str]
    exclude: list[str]
    model: str
    context: list[str]
    direction: list[str]
    area: str | None = None


def parse_step_file(name: str, text: str) -> StepFile:
    """Parse a step file, extracting frontmatter if present."""
    match = _FRONTMATTER_PATTERN.match(text)
    if not match:
        return StepFile(name=name, content=text, config=StepConfig())

    frontmatter = match.group(1)
    content = text[match.end() :]
    config_dict = _parse_yaml_frontmatter(frontmatter)

    return StepFile(
        name=name,
        content=content,
        config=StepConfig.model_validate(config_dict),
    )


def _parse_yaml_frontmatter(text: str) -> dict[str, Any]:
    """Parse simple YAML frontmatter.

    Handles:
    - key: value pairs
    - key: [inline, list] syntax
    - key:\n  - list\n  - items syntax
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


def get_step_defaults() -> StepConfig:
    """Return default step configuration."""
    return StepConfig(
        interactive=False,
        include=None,
        exclude=["tests/**"],
        model=None,
    )


def resolve_step_config(
    step_name: str,
    global_config,  # Config | None - avoid circular import
    frontmatter: StepConfig,
    cli_interactive: bool | None,
    cli_auto: bool | None,
    cli_model: str | None,
    cli_context: list[str] | None,
    cli_direction: list[str] | None = None,
    cli_area: str | None = None,
) -> ResolvedStepConfig:
    """Merge configs: CLI > frontmatter > global > defaults."""
    defaults = get_step_defaults()

    # Resolve interactive: CLI > frontmatter > global (interactive list) > default
    if cli_interactive:
        interactive = True
    elif cli_auto:
        interactive = False
    elif frontmatter.interactive is not None:
        interactive = frontmatter.interactive
    elif global_config and step_name in global_config.interactive:
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
    elif defaults.exclude is not None:
        exclude = list(defaults.exclude)
    else:
        exclude = []

    # Resolve include: frontmatter > default
    if frontmatter.include is not None:
        include = list(frontmatter.include)
    elif defaults.include is not None:
        include = list(defaults.include)
    else:
        include = []

    # Resolve direction: CLI > frontmatter > global > default
    if cli_direction:
        direction = list(cli_direction)
    elif frontmatter.direction:
        direction = list(frontmatter.direction)
    elif global_config and global_config.direction:
        direction = list(global_config.direction)
    else:
        direction = []

    # Resolve context: global context + frontmatter + CLI
    context: list[str] = []
    if global_config and global_config.context:
        context.extend(global_config.context)
    if frontmatter.context:
        context.extend(frontmatter.context)
    if cli_context:
        context.extend(cli_context)

    # Remove included paths from excludes
    if include:
        exclude = [item for item in exclude if item not in include]

    # Resolve area: CLI > frontmatter > global > default
    if cli_area:
        area = cli_area
    elif frontmatter.area:
        area = frontmatter.area
    elif global_config and hasattr(global_config, "area") and global_config.area:
        area = global_config.area
    else:
        area = None

    return ResolvedStepConfig(
        interactive=interactive,
        include=include,
        exclude=exclude,
        model=model,
        context=context,
        direction=direction,
        area=area,
    )
