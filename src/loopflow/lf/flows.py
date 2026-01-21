"""Flow DAG loading and execution for agents."""

from dataclasses import dataclass
from importlib import util as importlib_util
from pathlib import Path
from types import ModuleType
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, model_validator

from loopflow.lf.frontmatter import StepConfig


class Flow(list):
    """Convenience wrapper for flow step lists."""

    def __init__(self, *steps):
        if len(steps) == 1:
            value = steps[0]
            if isinstance(value, str):
                super().__init__([value])
                return
            if isinstance(value, (list, tuple)):
                super().__init__(value)
                return
        super().__init__(steps)


class RaceConfig(BaseModel):
    """Configuration for model racing—run same task with multiple models."""

    model_config = ConfigDict(extra="forbid")

    models: list[str] = Field(default_factory=list)
    judge: str = "compare"

    @model_validator(mode="before")
    @classmethod
    def _normalize(cls, data):
        if isinstance(data, list):
            models: list[str] = []
            for item in data:
                if isinstance(item, str):
                    models.append(item)
                elif isinstance(item, dict) and "model" in item:
                    models.append(item["model"])
            return {"models": models}
        return data

    def to_dict(self) -> dict:
        return self.model_dump(exclude_none=True)

    @classmethod
    def from_dict(cls, data: list | dict) -> "RaceConfig":
        return cls.model_validate(data)


class ChooseFork(BaseModel):
    """Prompt-driven fork between named subflows."""

    model_config = ConfigDict(extra="forbid")

    options: dict[str, list[Any]]
    output: str | None = None
    prompt: str | None = None


class ChooseResultOption(BaseModel):
    """Variant configuration for choose_result."""

    model_config = ConfigDict(extra="forbid")

    model: str
    voice: list[str] | None = None
    context: list[str] | None = None
    label: str | None = None


class ChooseResult(BaseModel):
    """Run variants and select the best result."""

    model_config = ConfigDict(extra="forbid")

    step: str
    options: list[ChooseResultOption]
    judge: str = "compare"
    output: str | None = None


class FlowStep(BaseModel):
    model_config = ConfigDict(extra="forbid")

    step: str | None = None
    flow: str | None = None
    parallel: list["FlowStep"] | None = None
    race: RaceConfig | None = None
    config: StepConfig | None = None
    choose_fork: ChooseFork | None = None
    choose_result: ChooseResult | None = None

    @model_validator(mode="before")
    @classmethod
    def _normalize(cls, data):
        if isinstance(data, str):
            return {"step": data}
        if isinstance(data, ChooseFork):
            return {"choose_fork": data}
        if isinstance(data, ChooseResult):
            return {"choose_result": data}
        return data

    def to_dict(self) -> dict | str:
        return _step_to_data(self)

    @classmethod
    def from_dict(cls, data: dict | str) -> "FlowStep":
        return cls.model_validate(data)


class FlowDef(BaseModel):
    model_config = ConfigDict(extra="forbid")

    name: str
    steps: list[FlowStep]

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "steps": [_step_to_data(step) for step in self.steps],
        }

    @classmethod
    def from_dict(cls, name: str, data: dict) -> "FlowDef":
        payload = {"name": name, **data}
        return cls.model_validate(payload)


FlowStep.model_rebuild()


def _step_to_data(step: FlowStep) -> dict | str:
    if step.choose_fork:
        return {"choose_fork": _choose_fork_to_data(step.choose_fork)}
    if step.choose_result:
        return {"choose_result": _choose_result_to_data(step.choose_result)}
    if step.parallel is not None:
        return {"parallel": [_step_to_data(s) for s in step.parallel]}

    if step.flow:
        data: dict = {"flow": step.flow}
    elif step.step:
        data = {"step": step.step}
    else:
        return {}

    if step.race:
        data["race"] = step.race.to_dict()
    if step.config:
        config_data = step.config.to_dict()
        if config_data:
            data["config"] = config_data

    if data == {"step": step.step}:
        return step.step or ""

    return data


def _choose_fork_to_data(choose_fork: ChooseFork) -> dict:
    return choose_fork.model_dump(exclude_none=True)


def _choose_result_to_data(choose_result: ChooseResult) -> dict:
    return choose_result.model_dump(exclude_none=True)


def _load_flow_module(name: str, path: Path) -> ModuleType:
    spec = importlib_util.spec_from_file_location(f"loopflow.flow.{name}", path)
    if not spec or not spec.loader:
        raise ValueError(f"Flow '{name}' failed to load")

    module = importlib_util.module_from_spec(spec)
    module.__dict__["Flow"] = Flow
    module.__dict__["ChooseFork"] = ChooseFork
    module.__dict__["ChooseResult"] = ChooseResult
    spec.loader.exec_module(module)
    return module


def _coerce_flow(name: str, data: Any) -> FlowDef:
    if isinstance(data, FlowDef):
        return data
    if isinstance(data, list):
        return FlowDef.from_dict(name, {"steps": data})
    if isinstance(data, dict):
        return FlowDef.from_dict(name, data)
    raise ValueError(f"Flow '{name}' must return FlowDef, dict, or list")


def load_flow(name: str, repo: Path) -> FlowDef | None:
    """Load flow from .lf/flows/{name}.py."""
    flow_path = repo / ".lf" / "flows" / f"{name}.py"
    if not flow_path.exists():
        return None

    module = _load_flow_module(name, flow_path)
    flow_func = getattr(module, "flow", None)
    if callable(flow_func):
        return _coerce_flow(name, flow_func())

    flow_value = getattr(module, name.upper(), None)
    if flow_value is None:
        flow_value = getattr(module, "FLOW", None)
    if flow_value is None:
        raise ValueError(f"Flow '{name}' must define flow() or FLOW/{name.upper()}")

    return _coerce_flow(name, flow_value)


def save_flow(flow: FlowDef, repo: Path) -> Path:
    """Save flow to .lf/flows/{name}.py. Returns the path."""
    flows_dir = repo / ".lf" / "flows"
    flows_dir.mkdir(parents=True, exist_ok=True)

    flow_path = flows_dir / f"{flow.name}.py"
    data = {"steps": [_step_to_data(step) for step in flow.steps]}
    contents = """# Generated by loopflow. Edit to customize.

def flow():
    return {data}
""".format(data=repr(data))
    flow_path.write_text(contents)

    return flow_path


def list_flows(repo: Path) -> list[FlowDef]:
    """List all flows in .lf/flows/."""
    flows_dir = repo / ".lf" / "flows"
    if not flows_dir.exists():
        return []

    flows = []
    for path in flows_dir.glob("*.py"):
        name = path.stem
        flow = load_flow(name, repo)
        if flow:
            flows.append(flow)

    return flows


@dataclass
class ResolvedStep:
    """A step ready for execution with dependencies resolved."""

    step: str | None = None
    config: StepConfig | None = None
    parallel_group: int | None = None
    race: RaceConfig | None = None
    choose_fork: ChooseFork | None = None
    choose_result: ChooseResult | None = None


def resolve_flow(flow: FlowDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested flows, return flat list with parallel groups marked."""
    resolved: list[ResolvedStep] = []
    parallel_group = 0

    def _resolve_step(flow_step: FlowStep, group: int | None = None) -> None:
        nonlocal parallel_group

        if flow_step.choose_fork:
            resolved.append(
                ResolvedStep(
                    choose_fork=flow_step.choose_fork,
                    parallel_group=group,
                )
            )
        elif flow_step.choose_result:
            resolved.append(
                ResolvedStep(
                    choose_result=flow_step.choose_result,
                    parallel_group=group,
                )
            )
        elif flow_step.step:
            resolved.append(
                ResolvedStep(
                    step=flow_step.step,
                    config=flow_step.config,
                    parallel_group=group,
                    race=flow_step.race,
                )
            )
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
