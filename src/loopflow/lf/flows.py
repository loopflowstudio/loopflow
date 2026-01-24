"""Flow DAG loading and execution for agents."""

from dataclasses import dataclass
from dataclasses import field as dataclass_field
from importlib import util as importlib_util
from pathlib import Path
from types import ModuleType
from typing import Any, Iterable

from pydantic import BaseModel, ConfigDict, model_validator

MAX_FORK_AGENTS = 5


@dataclass
class Step:
    """A step with optional overrides and dependencies."""

    name: str
    after: str | list[str] | None = None  # None = follows previous step
    model: str | None = None
    goal: str | None = None


@dataclass
class ForkAgent:
    """Configuration for one agent in a Fork."""

    step: str | None = None  # single step
    flow: str | None = None  # or full flow
    goal: str | None = None
    model: str | None = None
    area: str | None = None  # defaults to parent's area


@dataclass
class SynthesizeConfig:
    """Config for synthesis after fork."""

    goal: str | None = None
    area: str | None = None
    prompt: str | None = None


@dataclass
class Fork:
    """Spawn parallel agents with synthesis."""

    agents: list[ForkAgent] = dataclass_field(default_factory=list)
    step: str | None = None  # apply to all agents
    model: str | None = None  # apply to all agents
    synthesize: SynthesizeConfig | None = None

    def __init__(
        self,
        *agents,
        step: str | None = None,
        model: str | None = None,
        synthesize: dict | None = None,
    ):
        parsed = []
        for agent in agents:
            parsed.append(_parse_fork_agent(agent))
        if len(parsed) > MAX_FORK_AGENTS:
            raise ValueError(f"Fork limited to {MAX_FORK_AGENTS} agents, got {len(parsed)}")
        self.agents = parsed
        self.step = step
        self.model = model
        self.synthesize = SynthesizeConfig(**synthesize) if synthesize else None


class Flow(list):
    """Convenience wrapper for flow step lists with DAG support."""

    def __init__(self, *steps):
        parsed = []
        for step in steps:
            parsed.append(_parse_flow_item(step))
        super().__init__(parsed)


class Choose(BaseModel):
    """Prompt-driven choice between named subflows."""

    model_config = ConfigDict(extra="forbid")

    options: dict[str, list[Any]]
    output: str | None = None
    prompt: str | None = None

    @model_validator(mode="after")
    def _normalize(self):
        normalized = {}
        for key, value in self.options.items():
            normalized[key] = _parse_flow_items(value)
        self.options = normalized
        return self


FlowItem = Step | Fork | Choose


@dataclass
class FlowDef:
    name: str
    steps: list[FlowItem]

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "steps": [_step_to_data(step) for step in self.steps],
        }

    @classmethod
    def from_dict(cls, name: str, data: dict) -> "FlowDef":
        steps = _parse_flow_items(data.get("steps", []))
        return cls(name=name, steps=steps)


@dataclass(frozen=True)
class StepDAG:
    steps: dict[str, Step]
    dependencies: dict[str, set[str]]
    order: list[str]


def _parse_flow_items(items: Iterable[Any]) -> list[FlowItem]:
    return [_parse_flow_item(item) for item in items]


def _parse_flow_item(item: Any) -> FlowItem:
    if isinstance(item, (Step, Fork, Choose)):
        return item
    if isinstance(item, str):
        return Step(name=item)
    if isinstance(item, dict):
        if "choose" in item:
            choose_value = item["choose"]
            if isinstance(choose_value, Choose):
                return choose_value
            return Choose.model_validate(choose_value)
        if "fork" in item:
            agents = item["fork"]
            if not isinstance(agents, list):
                raise ValueError("fork must be a list of agents")
            return Fork(
                *agents,
                step=item.get("step"),
                model=item.get("model"),
                synthesize=item.get("synthesize"),
            )
        if "step" in item or "name" in item:
            name = item.get("name") or item.get("step")
            return Step(
                name=name,
                after=item.get("after"),
                model=item.get("model"),
                goal=item.get("goal"),
            )
    raise ValueError(f"Unsupported flow item: {item!r}")


def _parse_fork_agent(agent: Any) -> ForkAgent:
    if isinstance(agent, ForkAgent):
        return agent
    if isinstance(agent, dict):
        return ForkAgent(**agent)
    raise ValueError(f"Fork agent must be dict or ForkAgent, got {type(agent)}")


def _step_to_data(step: FlowItem) -> dict | str:
    if isinstance(step, Step):
        if not step.after and not step.model and not step.goal:
            return step.name
        data: dict[str, Any] = {"step": step.name}
        if step.after:
            data["after"] = step.after
        if step.model:
            data["model"] = step.model
        if step.goal:
            data["goal"] = step.goal
        return data
    if isinstance(step, Fork):
        result: dict[str, Any] = {
            "fork": [
                {
                    "step": agent.step,
                    "flow": agent.flow,
                    "goal": agent.goal,
                    "model": agent.model,
                    "area": agent.area,
                }
                for agent in step.agents
            ]
        }
        if step.step:
            result["step"] = step.step
        if step.model:
            result["model"] = step.model
        if step.synthesize:
            result["synthesize"] = {
                "goal": step.synthesize.goal,
                "area": step.synthesize.area,
                "prompt": step.synthesize.prompt,
            }
        return result
    if isinstance(step, Choose):
        return {"choose": step.model_dump(exclude_none=True)}
    raise ValueError(f"Unsupported step type: {type(step)}")


def build_step_dag(steps: list[Step]) -> StepDAG:
    """Build a dependency graph for a list of steps."""
    names = []
    seen = set()
    for step in steps:
        if step.name in seen:
            raise ValueError(f"Duplicate step name: {step.name}")
        seen.add(step.name)
        names.append(step.name)

    dependencies: dict[str, set[str]] = {}
    previous: str | None = None
    for step in steps:
        deps: set[str] = set()
        if step.after is None:
            if previous:
                deps.add(previous)
        else:
            after_list = [step.after] if isinstance(step.after, str) else list(step.after)
            deps.update(after_list)
        dependencies[step.name] = deps
        previous = step.name

    unknown = {dep for deps in dependencies.values() for dep in deps if dep not in seen}
    if unknown:
        unknown_list = ", ".join(sorted(unknown))
        raise ValueError(f"Unknown dependencies in flow: {unknown_list}")

    return StepDAG(
        steps={step.name: step for step in steps},
        dependencies=dependencies,
        order=names,
    )


def _load_flow_module(name: str, path: Path) -> ModuleType:
    spec = importlib_util.spec_from_file_location(f"loopflow.flow.{name}", path)
    if not spec or not spec.loader:
        raise ValueError(f"Flow '{name}' failed to load")

    module = importlib_util.module_from_spec(spec)
    # Inject flow primitives for backwards compat (prefer explicit imports)
    module.__dict__["Flow"] = Flow
    module.__dict__["Step"] = Step
    module.__dict__["Fork"] = Fork
    module.__dict__["ForkAgent"] = ForkAgent
    module.__dict__["Choose"] = Choose
    spec.loader.exec_module(module)
    return module


def _coerce_flow(name: str, data: Any) -> FlowDef:
    if isinstance(data, FlowDef):
        return data
    if isinstance(data, Flow):
        return FlowDef(name=name, steps=list(data))
    if isinstance(data, list):
        steps = _parse_flow_items(data)
        return FlowDef(name=name, steps=steps)
    if isinstance(data, dict):
        return FlowDef.from_dict(name, data)
    raise ValueError(f"Flow '{name}' must return FlowDef, dict, or list")


def _get_builtins_dir() -> Path:
    return Path(__file__).parent / "builtins" / "flows"


def load_flow(name: str, repo: Path | None) -> FlowDef | None:
    """Load flow from flows/{name}.py (repo, global, then builtins)."""
    flow_path = None

    if repo:
        repo_flow = repo / ".lf" / "flows" / f"{name}.py"
        if repo_flow.exists():
            flow_path = repo_flow

    if not flow_path:
        global_flow = Path.home() / ".lf" / "flows" / f"{name}.py"
        if global_flow.exists():
            flow_path = global_flow

    if not flow_path:
        builtin_flow = _get_builtins_dir() / f"{name}.py"
        if builtin_flow.exists():
            flow_path = builtin_flow

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


def list_flows(repo: Path | None) -> list[FlowDef]:
    """List all flows (repo, global, builtins)."""
    seen = set()
    flows = []

    if repo:
        repo_flows_dir = repo / ".lf" / "flows"
        if repo_flows_dir.exists():
            for path in repo_flows_dir.glob("*.py"):
                name = path.stem
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    global_flows_dir = Path.home() / ".lf" / "flows"
    if global_flows_dir.exists():
        for path in global_flows_dir.glob("*.py"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    builtins_dir = _get_builtins_dir()
    if builtins_dir.exists():
        for path in builtins_dir.glob("*.py"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)

    return flows


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
