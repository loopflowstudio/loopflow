"""Flow DAG loading and execution for agents."""

from dataclasses import dataclass
from dataclasses import field as dataclass_field
from pathlib import Path
from typing import Any, Iterable

import yaml
from pydantic import BaseModel, ConfigDict, model_validator

MAX_FORK_AGENTS = 5


@dataclass
class Step:
    """A step with optional overrides and dependencies."""

    name: str
    after: str | list[str] | None = None  # None = follows previous step
    model: str | None = None
    direction: str | None = None


@dataclass
class ForkAgent:
    """Configuration for one agent in a Fork."""

    step: str | None = None  # single step
    flow: str | None = None  # or full flow
    direction: str | None = None
    model: str | None = None
    area: str | None = None  # defaults to parent's area


@dataclass
class SynthesizeConfig:
    """Config for synthesis after fork."""

    direction: str | None = None
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


class Flow:
    """A flow is a sequence of steps.

    Can be constructed two ways:
    - Flow("implement", "reduce", "polish")  # convenience
    - Flow(name="ship", steps=[...])         # explicit

    Parsing is deferred until steps are accessed.
    """

    def __init__(self, *args, name: str = "", steps: list | None = None):
        self.name = name
        if steps is not None:
            self._steps = steps
            self._raw = None
        else:
            self._steps = None
            self._raw = args

    @property
    def steps(self) -> list[FlowItem]:
        if self._steps is None:
            self._steps = _parse_flow_items(self._raw)
        return self._steps

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "steps": [_step_to_data(step) for step in self.steps],
        }

    @classmethod
    def from_dict(cls, name: str, data: dict) -> "Flow":
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
            fork_value = item["fork"]
            if isinstance(fork_value, dict):
                # Nested structure: fork: { step, drafts, ... }
                drafts = fork_value.get("drafts", [])
                return Fork(
                    *drafts,
                    step=fork_value.get("step"),
                    model=fork_value.get("model"),
                    synthesize=fork_value.get("synthesize"),
                )
            else:
                # Flat structure (legacy): fork: [...], step: ...
                if not isinstance(fork_value, list):
                    raise ValueError("fork must be a list or dict")
                return Fork(
                    *fork_value,
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
                direction=item.get("direction"),
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
        if not step.after and not step.model and not step.direction:
            return step.name
        data: dict[str, Any] = {"step": step.name}
        if step.after:
            data["after"] = step.after
        if step.model:
            data["model"] = step.model
        if step.direction:
            data["direction"] = step.direction
        return data
    if isinstance(step, Fork):
        result: dict[str, Any] = {
            "fork": [
                {
                    "step": agent.step,
                    "flow": agent.flow,
                    "direction": agent.direction,
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
                "direction": step.synthesize.direction,
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


def _load_flow_yaml(name: str, path: Path) -> Flow:
    """Load a flow from a YAML file."""
    data = yaml.safe_load(path.read_text())
    if isinstance(data, list):
        return Flow(*data, name=name)
    if isinstance(data, dict):
        return Flow.from_dict(name, data)
    raise ValueError(f"Flow '{name}' must be a list or dict in YAML")


def _get_builtins_dir() -> Path:
    return Path(__file__).parent / "builtins" / "flows"


def _step_exists(name: str, repo: Path | None) -> bool:
    """Check if a step exists (repo, global, or builtin)."""
    from loopflow.lf.context import gather_step

    return gather_step(repo, name) is not None


def flow_file_exists(name: str, repo: Path | None) -> bool:
    """Check if an actual flow file exists (repo, global, or builtins).

    Unlike load_flow, this does NOT consider autopromoted steps.
    """
    if repo:
        repo_flow = repo / ".lf" / "flows" / f"{name}.yaml"
        if repo_flow.exists():
            return True

    global_flow = Path.home() / ".lf" / "flows" / f"{name}.yaml"
    if global_flow.exists():
        return True

    builtin_flow = _get_builtins_dir() / f"{name}.yaml"
    if builtin_flow.exists():
        return True

    return False


def load_flow(name: str, repo: Path | None) -> Flow | None:
    """Load flow from flows/{name}.yaml (repo, global, then builtins).

    If no flow exists but a step with that name does, autopromote to single-step flow.
    """
    flow_path = None

    if repo:
        repo_flow = repo / ".lf" / "flows" / f"{name}.yaml"
        if repo_flow.exists():
            flow_path = repo_flow

    if not flow_path:
        global_flow = Path.home() / ".lf" / "flows" / f"{name}.yaml"
        if global_flow.exists():
            flow_path = global_flow

    if not flow_path:
        builtin_flow = _get_builtins_dir() / f"{name}.yaml"
        if builtin_flow.exists():
            flow_path = builtin_flow

    # Autopromote: if no flow but step exists, create single-step flow
    if not flow_path:
        if _step_exists(name, repo):
            return Flow(name=name, steps=[Step(name=name)])
        return None

    return _load_flow_yaml(name, flow_path)


def list_flows(repo: Path | None) -> list[Flow]:
    """List all flows (repo, global, builtins)."""
    seen = set()
    flows = []

    if repo:
        repo_flows_dir = repo / ".lf" / "flows"
        if repo_flows_dir.exists():
            for path in repo_flows_dir.glob("*.yaml"):
                name = path.stem
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    global_flows_dir = Path.home() / ".lf" / "flows"
    if global_flows_dir.exists():
        for path in global_flows_dir.glob("*.yaml"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    builtins_dir = _get_builtins_dir()
    if builtins_dir.exists():
        for path in builtins_dir.glob("*.yaml"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)

    return flows


def list_steps(repo: Path | None) -> list[str]:
    """List all step names (repo, global, builtins)."""
    seen = set()
    steps = []

    # Repo steps
    if repo:
        for steps_dir in [repo / ".lf" / "steps", repo / ".claude" / "commands"]:
            if steps_dir.exists():
                for path in steps_dir.glob("*.md"):
                    name = path.stem
                    if name not in seen:
                        steps.append(name)
                        seen.add(name)

    # Global steps
    for global_dir in [
        Path.home() / ".lf" / "steps",
        Path.home() / ".claude" / "commands",
    ]:
        if global_dir.exists():
            for path in global_dir.glob("*.md"):
                name = path.stem
                if name not in seen:
                    steps.append(name)
                    seen.add(name)

    # Builtin steps
    builtins_steps = Path(__file__).parent / "builtins" / "steps"
    if builtins_steps.exists():
        for path in builtins_steps.glob("*.md"):
            name = path.stem
            if name not in seen:
                steps.append(name)
                seen.add(name)

    return sorted(steps)


def save_flow(flow: Flow, repo: Path) -> Path:
    """Save flow to .lf/flows/{name}.yaml. Returns the path."""
    flows_dir = repo / ".lf" / "flows"
    flows_dir.mkdir(parents=True, exist_ok=True)

    flow_path = flows_dir / f"{flow.name}.yaml"
    data = [_step_to_data(step) for step in flow.steps]
    flow_path.write_text(yaml.dump(data, default_flow_style=False, sort_keys=False))

    return flow_path
