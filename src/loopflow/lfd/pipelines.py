"""Pipeline DAG loading and execution for agents."""

from dataclasses import dataclass
from pathlib import Path

import yaml


@dataclass
class StepConfig:
    model: str | None = None
    voice: str | None = None
    context: list[str] | None = None

    def to_dict(self) -> dict:
        result = {}
        if self.model:
            result["model"] = self.model
        if self.voice:
            result["voice"] = self.voice
        if self.context:
            result["context"] = self.context
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "StepConfig":
        return cls(
            model=data.get("model"),
            voice=data.get("voice"),
            context=data.get("context"),
        )


@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None
    parallel: list["PipelineStep"] | None = None
    config: StepConfig | None = None

    def to_dict(self) -> dict:
        result = {}
        if self.task:
            result["task"] = self.task
        if self.pipeline:
            result["pipeline"] = self.pipeline
        if self.parallel:
            result["parallel"] = [s.to_dict() for s in self.parallel]
        if self.config:
            result["config"] = self.config.to_dict()
        return result

    @classmethod
    def from_dict(cls, data: dict | str) -> "PipelineStep":
        if isinstance(data, str):
            return cls(task=data)

        parallel_data = data.get("parallel")
        parallel = [cls.from_dict(s) for s in parallel_data] if parallel_data else None

        config_data = data.get("config")
        config = StepConfig.from_dict(config_data) if config_data else None

        return cls(
            task=data.get("task"),
            pipeline=data.get("pipeline"),
            parallel=parallel,
            config=config,
        )


@dataclass
class PipelineDef:
    name: str
    steps: list[PipelineStep]

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "steps": [s.to_dict() for s in self.steps],
        }

    @classmethod
    def from_dict(cls, name: str, data: dict) -> "PipelineDef":
        steps_data = data.get("steps", [])
        steps = [PipelineStep.from_dict(s) for s in steps_data]
        return cls(name=name, steps=steps)


def load_pipeline(name: str, repo: Path) -> PipelineDef | None:
    """Load pipeline from .lf/pipelines/{name}.yaml."""
    pipeline_path = repo / ".lf" / "pipelines" / f"{name}.yaml"
    if not pipeline_path.exists():
        return None

    data = yaml.safe_load(pipeline_path.read_text())
    if not data:
        return None

    return PipelineDef.from_dict(name, data)


def save_pipeline(pipeline: PipelineDef, repo: Path) -> Path:
    """Save pipeline to .lf/pipelines/{name}.yaml. Returns the path."""
    pipelines_dir = repo / ".lf" / "pipelines"
    pipelines_dir.mkdir(parents=True, exist_ok=True)

    pipeline_path = pipelines_dir / f"{pipeline.name}.yaml"
    data = {"steps": [s.to_dict() for s in pipeline.steps]}
    pipeline_path.write_text(yaml.dump(data, default_flow_style=False, sort_keys=False))

    return pipeline_path


def list_pipelines(repo: Path) -> list[PipelineDef]:
    """List all pipelines in .lf/pipelines/."""
    pipelines_dir = repo / ".lf" / "pipelines"
    if not pipelines_dir.exists():
        return []

    pipelines = []
    for path in pipelines_dir.glob("*.yaml"):
        name = path.stem
        pipeline = load_pipeline(name, repo)
        if pipeline:
            pipelines.append(pipeline)

    return pipelines


@dataclass
class ResolvedStep:
    """A step ready for execution with dependencies resolved."""
    task: str
    config: StepConfig | None = None
    parallel_group: int | None = None


def resolve_pipeline(pipeline: PipelineDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested pipelines, return flat list with parallel groups marked."""
    resolved: list[ResolvedStep] = []
    parallel_group = 0

    def _resolve_step(step: PipelineStep, group: int | None = None) -> None:
        nonlocal parallel_group

        if step.task:
            resolved.append(ResolvedStep(
                task=step.task,
                config=step.config,
                parallel_group=group,
            ))
        elif step.pipeline:
            nested = load_pipeline(step.pipeline, repo)
            if nested:
                for nested_step in nested.steps:
                    _resolve_step(nested_step, group)
        elif step.parallel:
            current_group = parallel_group
            parallel_group += 1
            for parallel_step in step.parallel:
                _resolve_step(parallel_step, current_group)

    for step in pipeline.steps:
        _resolve_step(step)

    return resolved
