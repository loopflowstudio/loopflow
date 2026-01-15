"""Configuration loading for loopflow."""

import warnings
from pathlib import Path
from typing import Optional

import yaml
from pydantic import BaseModel, Field, field_validator


class IdeConfig(BaseModel):
    warp: bool = True
    cursor: bool = True
    workspace: Optional[str] = None


class PipelineConfig(BaseModel):
    name: str = ""
    tasks: list[str]
    push: Optional[bool] = None
    pr: Optional[bool] = None


def parse_model(model: str) -> tuple[str, str | None]:
    """Parse model string like 'claude:opus' into (backend, variant).

    Applies smart defaults when no variant is specified:
    - claude -> opus (Claude Opus 4.5)
    - gemini -> 2.5-pro (Gemini 2.5 Pro)
    - codex -> None (let Codex CLI pick its default)
    """
    defaults = {
        "claude": "opus",
        "gemini": "2.5-pro",
    }
    parts = model.split(":", 1)
    backend = parts[0]
    variant = parts[1] if len(parts) > 1 else defaults.get(backend)
    return backend, variant


class Config(BaseModel):
    agent_model: str = "claude:opus"  # Format: backend:variant (e.g., claude:opus, claude:sonnet, codex)
    pipelines: dict[str, PipelineConfig] = Field(default_factory=dict)
    yolo: bool = False  # Skip all permission prompts
    push: bool = False
    pr: bool = False
    land: str = "gh"  # "gh" (GitHub PR merge) or "local" (local squash-merge)
    context: list[str] = Field(default_factory=list)
    exclude: list[str] = Field(default_factory=list)
    include_tests_for: Optional[list[str]] = None
    ide: IdeConfig = Field(default_factory=IdeConfig)
    interactive: list[str] = Field(default_factory=list)  # Tasks that default to interactive
    include_loopflow_doc: bool = True  # Include bundled LOOPFLOW.md in prompts
    docs: bool = True  # Include repo documentation (.md files)
    diff: bool = False  # Include raw branch diff against main
    diff_files: bool = True  # Include full content of files touched by branch
    paste: bool = False  # Include clipboard content by default
    voice: Optional[list[str]] = None  # Default voices for all tasks

    @field_validator("context", mode="before")
    @classmethod
    def split_context_string(cls, v):
        if isinstance(v, str):
            return v.split()
        return v

    @field_validator("exclude", mode="before")
    @classmethod
    def split_exclude_string(cls, v):
        if isinstance(v, str):
            return v.split()
        return v

    @field_validator("include_tests_for", mode="before")
    @classmethod
    def split_include_tests_for_string(cls, v):
        if isinstance(v, str):
            return v.split()
        return v

    @field_validator("voice", mode="before")
    @classmethod
    def normalize_voice(cls, v):
        if v is None:
            return None
        if isinstance(v, str):
            return [v] if v else None
        return v if v else None

    @field_validator("pipelines", mode="before")
    @classmethod
    def parse_pipelines(cls, v):
        if not v:
            return {}
        return {
            name: PipelineConfig(name=name, **data) if isinstance(data, dict) else data
            for name, data in v.items()
        }


class ConfigError(Exception):
    """User-friendly config error."""
    pass


def load_config(repo_root: Path) -> Config | None:
    """Load .lf/config.yaml. Returns None if not present."""
    config_path = repo_root / ".lf" / "config.yaml"
    if not config_path.exists():
        return None

    try:
        data = yaml.safe_load(config_path.read_text())
    except yaml.YAMLError as e:
        raise ConfigError(f"Invalid YAML in {config_path}:\n{e}")

    if not data:
        return None

    try:
        config = Config(**data)
    except Exception as e:
        # Extract the useful part from Pydantic errors
        msg = str(e)
        if "validation error" in msg.lower():
            # Simplify Pydantic's verbose output
            lines = msg.split("\n")
            errors = [
                l.strip() for l in lines[1:]
                if l.strip() and not l.strip().startswith("For further")
            ]
            raise ConfigError(f"Invalid config in {config_path}:\n" + "\n".join(errors))
        raise ConfigError(f"Invalid config in {config_path}: {e}")

    if config.include_tests_for:
        warnings.warn(
            "include_tests_for is deprecated. Use per-prompt frontmatter instead:\n"
            "---\n"
            "include:\n"
            "  - tests/**\n"
            "---",
            DeprecationWarning,
            stacklevel=2,
        )

    return config
