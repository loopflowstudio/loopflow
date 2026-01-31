"""Configuration loading for loopflow."""

from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import yaml
from pydantic import BaseModel, Field, field_validator

# Keys that combine lists from global + repo config
_ADDITIVE_KEYS = {"context", "exclude", "skill_sources", "summaries"}


@dataclass
class AutopruneConfig:
    """Auto-prune configuration for lfd daemon."""

    enabled: bool = False
    poll_interval_seconds: int = 60


class IdeConfig(BaseModel):
    warp: bool = True
    cursor: bool = True
    workspace: Optional[str] = None


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


class BudgetConfig(BaseModel):
    """Token budgets for prompt sections."""

    area: int = 50000  # Area content (area/*.md, area/reports/)
    docs: int = 30000  # Reference docs (root *.md, scratch/, reports/)
    diff: int = 20000  # Branch changes


class InternalConfig(BaseModel):
    """Internal/experimental settings."""

    use_rust: bool = False  # Use Rust lf-engine for git operations


def get_internal_flag(name: str, repo_root: Path | None = None) -> bool:
    """Check internal flag: env var (LF_{NAME}) first, then config.

    Returns False if neither is set.
    """
    import os

    env_name = f"LF_{name.upper()}"
    env = os.environ.get(env_name)
    if env is not None:
        return env == "1"

    config = load_config(repo_root)
    if config and config.internal:
        return getattr(config.internal, name, False)
    return False


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
    ide: IdeConfig = Field(default_factory=IdeConfig)
    interactive: list[str] = Field(default_factory=list)  # Tasks that default to interactive
    include_loopflow_doc: bool = True  # Include bundled LOOPFLOW.md in prompts
    lfdocs: bool = True  # Include reports/, roadmap/, scratch/, and root .md files
    diff: bool = False  # Include raw branch diff against main
    diff_files: bool = True  # Include full content of files touched by branch
    paste: bool = False  # Include clipboard content by default
    direction: Optional[list[str]] = None  # Default directions for all tasks
    area: Optional[str] = None  # Default area for parent doc inclusion (e.g., "lf/cli")
    summaries: list[SummaryConfig] = Field(default_factory=list)  # Summaries to include
    summary_tokens: int = 10000  # Default token budget for summaries
    skill_sources: list[SkillSourceConfig] = Field(default_factory=list)  # External skill libraries
    skill_registry: SkillRegistryConfig = Field(default_factory=SkillRegistryConfig)
    branch_names: Optional[BranchNameConfig] = None  # Branch naming schema
    lint_check: Optional[str] = None  # Command to check if lint passes (exits 0 = pass)
    autoprune: AutopruneConfig = Field(default_factory=AutopruneConfig)
    budgets: BudgetConfig = Field(default_factory=BudgetConfig)
    internal: InternalConfig = Field(default_factory=InternalConfig)

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

    @field_validator("direction", mode="before")
    @classmethod
    def normalize_direction(cls, v):
        if v is None:
            return None
        if isinstance(v, str):
            return [v] if v else None
        return v if v else None

    @field_validator("autoprune", mode="before")
    @classmethod
    def normalize_autoprune(cls, v):
        if v is True:
            return AutopruneConfig(enabled=True)
        if v is False or v is None:
            return AutopruneConfig(enabled=False)
        if isinstance(v, dict):
            return AutopruneConfig(**v)
        return v


class ConfigError(Exception):
    """User-friendly config error."""

    pass


def _load_yaml_file(path: Path) -> dict | None:
    """Load YAML file, returning None if not present or empty."""
    if not path.exists():
        return None
    try:
        data = yaml.safe_load(path.read_text())
    except yaml.YAMLError as e:
        raise ConfigError(f"Invalid YAML in {path}:\n{e}")
    return data if data else None


def _merge_config_dicts(global_cfg: dict | None, repo_cfg: dict | None) -> dict:
    """Merge global and repo config. Repo wins for scalars, additive keys combine."""
    if not global_cfg:
        return repo_cfg or {}
    if not repo_cfg:
        return global_cfg

    merged = {**global_cfg}
    for key, value in repo_cfg.items():
        if key in _ADDITIVE_KEYS and key in merged and merged[key]:
            merged[key] = merged[key] + value
        else:
            merged[key] = value
    return merged


def load_config(repo_root: Path | None) -> Config | None:
    """Load config, merging global (~/.lf/config.yaml) with repo (.lf/config.yaml)."""
    global_path = Path.home() / ".lf" / "config.yaml"
    repo_path = repo_root / ".lf" / "config.yaml" if repo_root else None

    global_data = _load_yaml_file(global_path)
    repo_data = _load_yaml_file(repo_path) if repo_path else None

    if not global_data and not repo_data:
        return None

    # Check for deprecated flows key in repo config
    if repo_data and "flows" in repo_data:
        raise ConfigError(
            f"Invalid config in {repo_path}:\n"
            "  'flows' is no longer supported in .lf/config.yaml.\n"
            "  Move flows to .lf/flows/<name>.py."
        )

    merged = _merge_config_dicts(global_data, repo_data)

    try:
        config = Config(**merged)
    except Exception as e:
        msg = str(e)
        if "validation error" in msg.lower():
            lines = msg.split("\n")
            errors = [
                line.strip()
                for line in lines[1:]
                if line.strip() and not line.strip().startswith("For further")
            ]
            source = repo_path if repo_data else global_path
            raise ConfigError(f"Invalid config in {source}:\n" + "\n".join(errors))
        raise ConfigError(f"Invalid config: {e}")

    return config
