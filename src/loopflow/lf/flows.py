"""Flow DAG loading and execution for agents."""

from dataclasses import dataclass
from pathlib import Path

import yaml

from loopflow.lf.frontmatter import StepConfig


@dataclass
class RaceConfig:
    """Configuration for model racing—run same task with multiple models."""
    models: list[str]
    judge: str = "compare"

    def to_dict(self) -> dict:
        result = {"models": self.models}
        if self.judge != "compare":
            result["judge"] = self.judge
        return result

    @classmethod
    def from_dict(cls, data: list | dict) -> "RaceConfig":
        if isinstance(data, list):
            # Simple list of models: race: [claude:opus, codex:o3]
            models = []
            for item in data:
                if isinstance(item, str):
                    models.append(item)
                elif isinstance(item, dict) and "model" in item:
                    models.append(item["model"])
            return cls(models=models)
        # Full config: race: {models: [...], judge: "..."}
        models = data.get("models", [])
        judge = data.get("judge", "compare")
        return cls(models=models, judge=judge)


@dataclass
class FlowStep:
    step: str | None = None
    flow: str | None = None
    parallel: list["FlowStep"] | None = None
    race: RaceConfig | None = None
    config: StepConfig | None = None

    def to_dict(self) -> dict:
        result = {}
        if self.step:
            result["step"] = self.step
        if self.flow:
            result["flow"] = self.flow
        if self.parallel:
            result["parallel"] = [s.to_dict() for s in self.parallel]
        if self.race:
            result["race"] = self.race.to_dict()
        if self.config:
            result["config"] = self.config.to_dict()
        return result

    @classmethod
    def from_dict(cls, data: dict | str) -> "FlowStep":
        if isinstance(data, str):
            return cls(step=data)

        parallel_data = data.get("parallel")
        parallel = [cls.from_dict(s) for s in parallel_data] if parallel_data else None

        race_data = data.get("race")
        race = RaceConfig.from_dict(race_data) if race_data else None

        config_data = data.get("config")
        config = StepConfig.from_dict(config_data) if config_data else None

        return cls(
            step=data.get("step"),
            flow=data.get("flow") or data.get("pipeline"),  # Backward compat
            parallel=parallel,
            race=race,
            config=config,
        )


@dataclass
class FlowDef:
    name: str
    steps: list[FlowStep]

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "steps": [s.to_dict() for s in self.steps],
        }

    @classmethod
    def from_dict(cls, name: str, data: dict) -> "FlowDef":
        steps_data = data.get("steps", [])
        steps = [FlowStep.from_dict(s) for s in steps_data]
        return cls(name=name, steps=steps)


def load_flow(name: str, repo: Path) -> FlowDef | None:
    """Load flow from .lf/flows/{name}.yaml."""
    flow_path = repo / ".lf" / "flows" / f"{name}.yaml"
    if not flow_path.exists():
        return None

    data = yaml.safe_load(flow_path.read_text())
    if not data:
        return None

    return FlowDef.from_dict(name, data)


def save_flow(flow: FlowDef, repo: Path) -> Path:
    """Save flow to .lf/flows/{name}.yaml. Returns the path."""
    flows_dir = repo / ".lf" / "flows"
    flows_dir.mkdir(parents=True, exist_ok=True)

    flow_path = flows_dir / f"{flow.name}.yaml"
    data = {"steps": [s.to_dict() for s in flow.steps]}
    flow_path.write_text(yaml.dump(data, default_flow_style=False, sort_keys=False))

    return flow_path


def list_flows(repo: Path) -> list[FlowDef]:
    """List all flows in .lf/flows/."""
    flows_dir = repo / ".lf" / "flows"
    if not flows_dir.exists():
        return []

    flows = []
    for path in flows_dir.glob("*.yaml"):
        name = path.stem
        flow = load_flow(name, repo)
        if flow:
            flows.append(flow)

    return flows


@dataclass
class ResolvedStep:
    """A step ready for execution with dependencies resolved."""
    step: str
    config: StepConfig | None = None
    parallel_group: int | None = None
    race: RaceConfig | None = None


def resolve_flow(flow: FlowDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested flows, return flat list with parallel groups marked."""
    resolved: list[ResolvedStep] = []
    parallel_group = 0

    def _resolve_step(flow_step: FlowStep, group: int | None = None) -> None:
        nonlocal parallel_group

        if flow_step.step:
            resolved.append(ResolvedStep(
                step=flow_step.step,
                config=flow_step.config,
                parallel_group=group,
                race=flow_step.race,
            ))
        elif flow_step.flow:
            nested = load_flow(flow_step.flow, repo)
            if nested:
                for nested_step in nested.steps:
                    _resolve_step(nested_step, group)
        elif flow_step.parallel:
            current_group = parallel_group
            parallel_group += 1
            for parallel_step in flow_step.parallel:
                _resolve_step(parallel_step, current_group)

    for flow_step in flow.steps:
        _resolve_step(flow_step)

    return resolved
