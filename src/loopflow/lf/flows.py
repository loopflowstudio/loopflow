"""Flow DAG loading and execution for agents."""

from dataclasses import dataclass
from dataclasses import field as dataclass_field
from pathlib import Path
from typing import Any, Iterable

import yaml
from pydantic import BaseModel, ConfigDict, model_validator

MAX_FORK_THREADS = 5


def _normalize(value: str | list[str] | None) -> list[str] | None:
    """Normalize str | list[str] | None to list[str] | None."""
    if value is None:
        return None
    if isinstance(value, str):
        return [value]
    return value


@dataclass
class Step:
    """A step with optional overrides and dependencies."""

    name: str
    after: str | list[str] | None = None  # None = follows previous step
    model: str | None = None
    direction: list[str] | None = None  # Overrides flow direction (normalized)
    interactive: bool | None = None  # Override frontmatter setting


@dataclass
class ForkThread:
    """Configuration for one parallel thread in a Fork."""

    step: str | None = None  # single step
    flow: str | None = None  # or full flow
    direction: list[str] | None = None  # normalized
    model: str | None = None
    area: list[str] | None = None  # normalized, defaults to parent's area


@dataclass
class SynthesizeConfig:
    """Config for synthesis after fork."""

    direction: list[str] | None = None  # normalized
    area: list[str] | None = None  # normalized
    prompt: str | None = None


@dataclass
class Fork:
    """Spawn parallel threads with synthesis."""

    threads: list[ForkThread] = dataclass_field(default_factory=list)
    step: str | None = None  # apply to all threads
    model: str | None = None  # apply to all threads
    synthesize: SynthesizeConfig | None = None

    def __init__(
        self,
        *threads,
        step: str | None = None,
        model: str | None = None,
        synthesize: dict | None = None,
    ):
        parsed = []
        for thread in threads:
            parsed.append(_parse_fork_thread(thread))
        if len(parsed) > MAX_FORK_THREADS:
            raise ValueError(f"Fork limited to {MAX_FORK_THREADS} threads, got {len(parsed)}")
        self.threads = parsed
        self.step = step
        self.model = model
        self.synthesize = (
            SynthesizeConfig(
                direction=_normalize(synthesize.get("direction")),
                area=_normalize(synthesize.get("area")),
                prompt=synthesize.get("prompt"),
            )
            if synthesize
            else None
        )


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


@dataclass
class LoopUntilEmpty:
    """Repeat steps until wave backlog is empty."""

    steps: list["FlowItem"] = dataclass_field(default_factory=list)
    wave: str | None = None  # None = inherit from context
    max_iterations: int = 100  # Safety limit


FlowItem = Step | Fork | Choose | LoopUntilEmpty


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
    if isinstance(item, (Step, Fork, Choose, LoopUntilEmpty)):
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
        if "loop_until_empty" in item:
            loop_value = item["loop_until_empty"]
            if isinstance(loop_value, dict):
                return LoopUntilEmpty(
                    steps=_parse_flow_items(loop_value.get("steps", [])),
                    wave=loop_value.get("wave"),
                    max_iterations=loop_value.get("max_iterations", 100),
                )
            raise ValueError("loop_until_empty must be a dict with 'steps' key")
        if "step" in item or "name" in item:
            name = item.get("name") or item.get("step")
            return Step(
                name=name,
                after=item.get("after"),
                model=item.get("model"),
                direction=_normalize(item.get("direction")),
                interactive=item.get("interactive"),
            )
    raise ValueError(f"Unsupported flow item: {item!r}")


def _parse_fork_thread(thread: Any) -> ForkThread:
    if isinstance(thread, ForkThread):
        return thread
    if isinstance(thread, dict):
        return ForkThread(
            step=thread.get("step"),
            flow=thread.get("flow"),
            direction=_normalize(thread.get("direction")),
            model=thread.get("model"),
            area=_normalize(thread.get("area")),
        )
    raise ValueError(f"Fork thread must be dict or ForkThread, got {type(thread)}")


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
                    "step": thread.step,
                    "flow": thread.flow,
                    "direction": thread.direction,
                    "model": thread.model,
                    "area": thread.area,
                }
                for thread in step.threads
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


def _find_flow_file(name: str, flows_dir: Path) -> Path | None:
    """Find a flow file by name in a directory (including subdirectories)."""
    # Check flat structure first
    flat = flows_dir / f"{name}.yaml"
    if flat.exists():
        return flat

    # Check subdirectories
    for path in flows_dir.glob(f"**/{name}.yaml"):
        return path

    return None


def flow_file_exists(name: str, repo: Path | None) -> bool:
    """Check if an actual flow file exists (repo, global, or builtins).

    Unlike load_flow, this does NOT consider autopromoted steps.
    """
    if repo:
        if _find_flow_file(name, repo / ".lf" / "flows"):
            return True

    if _find_flow_file(name, Path.home() / ".lf" / "flows"):
        return True

    if _find_flow_file(name, _get_builtins_dir()):
        return True

    return False


def load_flow(name: str, repo: Path | None) -> Flow | None:
    """Load flow from flows/{name}.yaml (repo, global, then builtins).

    Searches subdirectories. If no flow exists but a step with that name does,
    autopromote to single-step flow.
    """
    flow_path = None

    if repo:
        flow_path = _find_flow_file(name, repo / ".lf" / "flows")

    if not flow_path:
        flow_path = _find_flow_file(name, Path.home() / ".lf" / "flows")

    if not flow_path:
        flow_path = _find_flow_file(name, _get_builtins_dir())

    # Autopromote: if no flow but step exists, create single-step flow
    if not flow_path:
        if _step_exists(name, repo):
            return Flow(name=name, steps=[Step(name=name)])
        return None

    return _load_flow_yaml(name, flow_path)


def list_flows(repo: Path | None) -> list[Flow]:
    """List all flows (repo, global, builtins). Searches subdirectories."""
    seen = set()
    flows = []

    if repo:
        repo_flows_dir = repo / ".lf" / "flows"
        if repo_flows_dir.exists():
            for path in repo_flows_dir.glob("**/*.yaml"):
                name = path.stem
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    global_flows_dir = Path.home() / ".lf" / "flows"
    if global_flows_dir.exists():
        for path in global_flows_dir.glob("**/*.yaml"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)
                    seen.add(name)

    builtins_dir = _get_builtins_dir()
    if builtins_dir.exists():
        for path in builtins_dir.glob("**/*.yaml"):
            name = path.stem
            if name not in seen:
                flow = load_flow(name, repo)
                if flow:
                    flows.append(flow)

    return flows


def list_steps(repo: Path | None) -> list[str]:
    """List all step names (repo, global, builtins). Searches subdirectories."""
    seen = set()
    steps = []

    # Repo steps
    if repo:
        for steps_dir in [repo / ".lf" / "steps", repo / ".claude" / "commands"]:
            if steps_dir.exists():
                for path in steps_dir.glob("**/*.md"):
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
            for path in global_dir.glob("**/*.md"):
                name = path.stem
                if name not in seen:
                    steps.append(name)
                    seen.add(name)

    # Builtin steps
    builtins_steps = Path(__file__).parent / "builtins" / "steps"
    if builtins_steps.exists():
        for path in builtins_steps.glob("**/*.md"):
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


def list_directions(repo: Path | None) -> list[str]:
    """List all direction names (repo, global, builtins). Searches subdirectories."""
    seen = set()
    directions = []

    if repo:
        directions_dir = repo / ".lf" / "directions"
        if directions_dir.exists():
            for path in directions_dir.glob("**/*.md"):
                name = path.stem
                if name not in seen:
                    directions.append(name)
                    seen.add(name)

    global_dir = Path.home() / ".lf" / "directions"
    if global_dir.exists():
        for path in global_dir.glob("**/*.md"):
            name = path.stem
            if name not in seen:
                directions.append(name)
                seen.add(name)

    builtins_dir = Path(__file__).parent / "builtins" / "directions"
    if builtins_dir.exists():
        for path in builtins_dir.glob("**/*.md"):
            name = path.stem
            if name not in seen:
                directions.append(name)
                seen.add(name)

    return sorted(directions)


@dataclass
class ValidationError:
    flow_name: str
    item: str
    error: str


def _extract_step_names(flow: Flow) -> list[str]:
    """Extract all step names referenced in a flow."""
    names = []
    for item in flow.steps:
        if isinstance(item, Step):
            names.append(item.name)
            if item.direction:
                names.append(f"direction:{item.direction}")
        elif isinstance(item, Fork):
            if item.step:
                names.append(item.step)
            for thread in item.threads:
                if thread.step:
                    names.append(thread.step)
                if thread.flow:
                    names.append(f"flow:{thread.flow}")
                if thread.direction:
                    names.append(f"direction:{thread.direction}")
            if item.synthesize and item.synthesize.direction:
                names.append(f"direction:{item.synthesize.direction}")
        elif isinstance(item, Choose):
            for option_steps in item.options.values():
                for step in option_steps:
                    if isinstance(step, Step):
                        names.append(step.name)
    return names


def validate_flows(repo: Path | None) -> list[ValidationError]:
    """Validate all flows. Returns list of errors."""
    errors = []
    available_steps = set(list_steps(repo))
    available_directions = set(list_directions(repo))
    available_flows = {f.name for f in list_flows(repo)}

    for flow in list_flows(repo):
        refs = _extract_step_names(flow)
        for ref in refs:
            if ref.startswith("direction:"):
                direction = ref[10:]
                if direction not in available_directions:
                    errors.append(
                        ValidationError(
                            flow_name=flow.name, item=direction, error="direction not found"
                        )
                    )
            elif ref.startswith("flow:"):
                flow_ref = ref[5:]
                if flow_ref not in available_flows:
                    errors.append(
                        ValidationError(flow_name=flow.name, item=flow_ref, error="flow not found")
                    )
            else:
                # Could be step or flow (autopromote)
                if ref not in available_steps and ref not in available_flows:
                    errors.append(
                        ValidationError(flow_name=flow.name, item=ref, error="step/flow not found")
                    )

    return errors
