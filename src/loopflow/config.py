"""Configuration loading for loopflow."""

from dataclasses import dataclass, field
from pathlib import Path

import yaml

from loopflow.pipeline import Pipeline


@dataclass
class Config:
    pipelines: dict[str, Pipeline] = field(default_factory=dict)
    dangerously_skip_permissions: bool = False


def load_config(repo_root: Path) -> Config | None:
    """Load .lf/config.yaml. Returns None if not present."""
    config_path = repo_root / ".lf" / "config.yaml"
    if not config_path.exists():
        return None

    data = yaml.safe_load(config_path.read_text())
    if not data:
        return None

    pipelines = {}
    if "pipelines" in data:
        for name, tasks in data["pipelines"].items():
            pipelines[name] = Pipeline(name=name, tasks=tasks)

    dangerously_skip_permissions = data.get("dangerously_skip_permissions", False)

    return Config(pipelines=pipelines, dangerously_skip_permissions=dangerously_skip_permissions)
