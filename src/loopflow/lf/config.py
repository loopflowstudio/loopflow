"""Configuration loading for loopflow."""

import warnings
from pathlib import Path
from typing import Optional

import yaml
from pydantic import BaseModel, Field, field_validator, model_validator


class IdeConfig(BaseModel):
    warp: bool = True
    cursor: bool = True
    workspace: Optional[str] = None


class AsanaConfig(BaseModel):
    project_id: str


class WorkConfig(BaseModel):
    backend: str = "file"  # "file" or "asana"
    asana: Optional[AsanaConfig] = None
    auto_rebase: bool = True
    auto_land: bool = False


class SummaryConfig(BaseModel):
    """Per-path summary configuration."""

    path: str
    tokens: int | None = None  # Falls back to summary_tokens if not set
    model: str = "gemini"


class SkillSourceConfig(BaseModel):
    """External skill library configuration."""

    name: str
    prefix: str
    path: str  # Supports ~ expansion


class SkillRegistryConfig(BaseModel):
    """SkillRegistry configuration."""

    enabled: bool = False
    base_url: str = "https://skillregistry.io"
    prefix: str = "sr"
    cache_ttl_seconds: int = 86400
    cache_dir: Optional[str] = None


class BranchNameConfig(BaseModel):
    """Configuration for branch name generation."""

    schema_: str = Field(default="{name}", alias="schema")


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
    # Format: backend:variant (e.g., claude:opus, claude:sonnet, codex)
    agent_model: str = "claude:opus"
    yolo: bool = False  # Skip permissions; Codex also disables sandboxing
    chrome: bool = False  # Enable Chrome integration for Claude Code (browser automation)
    push: bool = False
    pr: bool = False
    land: str = "gh"  # "gh" (GitHub PR merge) or "local" (local squash-merge)
    context: list[str] = Field(default_factory=list)
    exclude: list[str] = Field(default_factory=list)
    ignore: list[str] = Field(default_factory=list)  # Alias for exclude, merged on load
    include_tests_for: Optional[list[str]] = None
    ide: IdeConfig = Field(default_factory=IdeConfig)
    interactive: list[str] = Field(default_factory=list)  # Tasks that default to interactive
    include_loopflow_doc: bool = True  # Include bundled LOOPFLOW.md in prompts
    lfdocs: bool = True  # Include .docs/, .design/, and root .md files
    diff: bool = False  # Include raw branch diff against main
    diff_files: bool = True  # Include full content of files touched by branch
    paste: bool = False  # Include clipboard content by default
    voice: Optional[list[str]] = None  # Default voices for all tasks
    summaries: list[SummaryConfig] = Field(default_factory=list)  # Summaries to include
    summary_tokens: int = 10000  # Default token budget for summaries
    skill_sources: list[SkillSourceConfig] = Field(default_factory=list)  # External skill libraries
    skill_registry: SkillRegistryConfig = Field(default_factory=SkillRegistryConfig)
    work: Optional[WorkConfig] = None  # Work queue configuration
    branch_names: Optional[BranchNameConfig] = None  # Branch naming schema
    lint_check: Optional[str] = None  # Command to check if lint passes (exits 0 = pass)

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

    @field_validator("ignore", mode="before")
    @classmethod
    def split_ignore_string(cls, v):
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

    @model_validator(mode="after")
    def merge_ignore_into_exclude(self) -> "Config":
        """Merge ignore into exclude (ignore is an alias)."""
        if self.ignore:
            self.exclude = list(set(self.exclude + self.ignore))
            self.ignore = []
        return self


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

    if "flows" in data:
        raise ConfigError(
            f"Invalid config in {config_path}:\n"
            "  'flows' is no longer supported in .lf/config.yaml.\n"
            "  Move flows to .lf/flows/<name>.py."
        )

    try:
        config = Config(**data)
    except Exception as e:
        # Extract the useful part from Pydantic errors
        msg = str(e)
        if "validation error" in msg.lower():
            # Simplify Pydantic's verbose output
            lines = msg.split("\n")
            errors = [
                line.strip()
                for line in lines[1:]
                if line.strip() and not line.strip().startswith("For further")
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
