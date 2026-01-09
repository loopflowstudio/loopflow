"""Configuration loading for loopflow."""

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


class Config(BaseModel):
    model: str = "claude"
    pipelines: dict[str, PipelineConfig] = Field(default_factory=dict)
    yolo: bool = False  # Skip all permission prompts
    push: bool = False
    pr: bool = False
    context: list[str] = Field(default_factory=list)
    exclude: list[str] = Field(default_factory=list)
    ide: IdeConfig = Field(default_factory=IdeConfig)

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
        return Config(**data)
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
