"""Configuration loading for loopflow."""

from dataclasses import dataclass, field
from pathlib import Path

import yaml

from loopflow.pipeline import Pipeline


@dataclass
class Config:
    pipelines: dict[str, Pipeline] = field(default_factory=dict)
    dangerously_skip_permissions: bool = False
    push: bool = False
    pr: bool = False
    context: list[str] = field(default_factory=list)


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
        for name, pipeline_data in data["pipelines"].items():
            pipelines[name] = Pipeline(
                name=name,
                tasks=pipeline_data["tasks"],
                push=pipeline_data.get("push"),
                pr=pipeline_data.get("pr"),
            )

    dangerously_skip_permissions = data.get("dangerously_skip_permissions", False)
    push = data.get("push", False)
    pr = data.get("pr", False)
    context_raw = data.get("context", [])
    if isinstance(context_raw, str):
        context_raw = context_raw.split()

    return Config(
        pipelines=pipelines,
        dangerously_skip_permissions=dangerously_skip_permissions,
        push=push,
        pr=pr,
        context=context_raw,
    )
