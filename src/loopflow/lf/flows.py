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


class JoinConfig(BaseModel):
    """Configuration for joining fork outcomes."""

    model_config = ConfigDict(extra="forbid")

    step: str | None = None
    agent_model: str | None = None
    voice: list[str] | None = None

    @model_validator(mode="before")
    @classmethod
    def _normalize(cls, data):
        if isinstance(data, str):
            return {"step": data}
        return data


class Choose(BaseModel):
    """Prompt-driven choice between named subflows."""

    model_config = ConfigDict(extra="forbid")

    options: dict[str, list[Any]]
    output: str | None = None
    prompt: str | None = None


class Join(BaseModel):
    """Join forked outputs into a single changeset."""

    model_config = ConfigDict(extra="forbid")

    join: JoinConfig = Field(default_factory=lambda: JoinConfig(step="synthesize"))


class FlowStep(BaseModel):
    model_config = ConfigDict(extra="forbid")

    step: str | None = None
    flow: str | None = None
    fork: list["FlowStep"] | None = None
    config: StepConfig | None = None
    choose: Choose | None = None
    join: Join | None = None

    @model_validator(mode="before")
    @classmethod
    def _normalize(cls, data):
        if isinstance(data, str):
            return {"step": data}
        if isinstance(data, Choose):
            return {"choose": data}
        if isinstance(data, Join):
            return {"join": data}
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
    if step.choose:
        return {"choose": _choose_to_data(step.choose)}
    if step.join:
        return {"join": _join_to_data(step.join)}
    if step.fork is not None:
        return {"fork": [_step_to_data(s) for s in step.fork]}

    if step.flow:
        data: dict = {"flow": step.flow}
    elif step.step:
        data = {"step": step.step}
    else:
        return {}

    if step.config:
        config_data = step.config.to_dict()
        if config_data:
            data["config"] = config_data

    if data == {"step": step.step}:
        return step.step or ""

    return data


def _choose_to_data(choose: Choose) -> dict:
    return choose.model_dump(exclude_none=True)


def _join_to_data(join: Join) -> dict:
    return join.model_dump(exclude_none=True)


def _load_flow_module(name: str, path: Path) -> ModuleType:
    spec = importlib_util.spec_from_file_location(f"loopflow.flow.{name}", path)
    if not spec or not spec.loader:
        raise ValueError(f"Flow '{name}' failed to load")

    module = importlib_util.module_from_spec(spec)
    module.__dict__["Flow"] = Flow
    module.__dict__["Choose"] = Choose
    module.__dict__["Join"] = Join
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


def load_flow(name: str, repo: Path | None) -> FlowDef | None:
    """Load flow from flows/{name}.py (repo then global)."""
    flow_path = None

    # Check repo first
    if repo:
        repo_flow = repo / ".lf" / "flows" / f"{name}.py"
        if repo_flow.exists():
            flow_path = repo_flow

    # Check global
    if not flow_path:
        global_flow = Path.home() / ".lf" / "flows" / f"{name}.py"
        if global_flow.exists():
            flow_path = global_flow

    if not flow_path:
        return None

    module = _load_flow_module(name, flow_path)
    flow_func = getattr(module, "flow", None)
    if callable(flow_func):
        return _coerce_flow(name, flow_func())

    flow_value = None
    for attr in (name.upper(), "FLOW"):
        flow_value = getattr(module, attr, None)
        if flow_value is not None:
            break
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


def list_flows(repo: Path | None) -> list[FlowDef]:
    """List all flows (repo and global)."""
    seen = set()
    flows = []

    # Repo flows
    if repo:
        repo_flows_dir = repo / ".lf" / "flows"
        if repo_flows_dir.exists():
            for path in repo_flows_dir.glob("*.py"):
                name = path.stem
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    # Global flows (not already in repo)
    global_flows_dir = Path.home() / ".lf" / "flows"
    if global_flows_dir.exists():
        for path in global_flows_dir.glob("*.py"):
            name = path.stem
            if name not in seen:
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
    choose: Choose | None = None
    join: Join | None = None


def resolve_flow(flow: FlowDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested flows, return flat list with fork groups marked."""
    resolved: list[ResolvedStep] = []
    parallel_group = 0

    def _resolve_step(flow_step: FlowStep, group: int | None = None) -> None:
        nonlocal parallel_group

        if flow_step.choose:
            resolved.append(
                ResolvedStep(
                    choose=flow_step.choose,
                    parallel_group=group,
                )
            )
        elif flow_step.join:
            resolved.append(
                ResolvedStep(
                    join=flow_step.join,
                    parallel_group=group,
                )
            )
        elif flow_step.step:
            resolved.append(
                ResolvedStep(
                    step=flow_step.step,
                    config=flow_step.config,
                    parallel_group=group,
                )
            )
        elif flow_step.flow:
            nested = load_flow(flow_step.flow, repo)
            if nested:
                for nested_step in nested.steps:
                    _resolve_step(nested_step, group)
        elif flow_step.fork:
            current_group = parallel_group
            parallel_group += 1
            for fork_step in flow_step.fork:
                _resolve_step(fork_step, current_group)

    for flow_step in flow.steps:
        _resolve_step(flow_step)

    return resolved
