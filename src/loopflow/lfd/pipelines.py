"""Pipeline DAG loading and execution for agents."""

import asyncio
from dataclasses import dataclass, field
from pathlib import Path

import yaml


@dataclass
class StepConfig:
    model: str | None = None

    def to_dict(self) -> dict:
        result = {}
        if self.model:
            result["model"] = self.model
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "StepConfig":
        return cls(model=data.get("model"))


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


@dataclass
class StepResult:
    task: str
    success: bool
    error: str | None = None


@dataclass
class PipelineResult:
    success: bool
    steps: list[StepResult] = field(default_factory=list)


async def execute_pipeline(
    pipeline: PipelineDef,
    repo: Path,
    worktree: Path,
    goal: str,
    run_step: callable,
) -> PipelineResult:
    """Run pipeline DAG in worktree, injecting goal as context for each step.

    run_step is a callable (task: str, worktree: Path, goal: str, config: StepConfig | None) -> bool
    that executes a single task and returns True on success.
    """
    resolved = resolve_pipeline(pipeline, repo)
    results: list[StepResult] = []

    i = 0
    while i < len(resolved):
        step = resolved[i]

        if step.parallel_group is not None:
            # Collect all steps in this parallel group
            parallel_steps = []
            group = step.parallel_group
            while i < len(resolved) and resolved[i].parallel_group == group:
                parallel_steps.append(resolved[i])
                i += 1

            # Run parallel steps concurrently
            step_results = await _run_parallel_steps(parallel_steps, worktree, goal, run_step)
            results.extend(step_results)

            if any(not r.success for r in step_results):
                return PipelineResult(success=False, steps=results)
        else:
            # Run sequential step
            success = await asyncio.to_thread(run_step, step.task, worktree, goal, step.config)
            result = StepResult(task=step.task, success=success)
            results.append(result)

            if not success:
                return PipelineResult(success=False, steps=results)

            i += 1

    return PipelineResult(success=True, steps=results)


async def _run_parallel_steps(
    steps: list[ResolvedStep],
    worktree: Path,
    goal: str,
    run_step: callable,
) -> list[StepResult]:
    """Run steps concurrently using asyncio."""

    async def run_one(step: ResolvedStep) -> StepResult:
        try:
            success = await asyncio.to_thread(run_step, step.task, worktree, goal, step.config)
            return StepResult(task=step.task, success=success)
        except Exception as e:
            return StepResult(task=step.task, success=False, error=str(e))

    results = await asyncio.gather(*[run_one(step) for step in steps])
    return list(results)
