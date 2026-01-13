# Agent Loop Design

## What to build

Add an "agent loop" system to Loopflow so Maestro can register background agents that run a configurable inner pipeline (design/implement/review/etc) with a persistent prompt and optional context.

## Data structures

```python
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class AgentLoopSpec:
    name: str
    prompt_path: Path
    pipeline: list[str]
    context: list[str]
    outer_loop: "OuterLoopConfig"


@dataclass
class OuterLoopConfig:
    mode: str  # "pr-chain" | "land-commits"
    # pr-chain: create PRs and chain dependencies
    # land-commits: land commits locally (or via lf land)


@dataclass
class RegisteredAgent:
    id: str
    spec: AgentLoopSpec
    status: str  # "idle" | "running" | "error"
    last_run_at: Optional[str]
```

## APIs

```python
# Maestro API
def register_agent(spec: AgentLoopSpec) -> RegisteredAgent: ...
def list_agents() -> list[RegisteredAgent]: ...
def start_agent(agent_id: str) -> None: ...
def stop_agent(agent_id: str) -> None: ...
```

## Constraints

- The "agent loop" must be registered through Maestro; it is a background agent with an inner pipeline.
- A prompt file defines the agent's overall goals and is included in all LLM requests.
- The pipeline is a sequence of commands like design -> implement -> review -> iterate -> expand -> polish.
- Outer loop behavior is TBD: "create PRs and chain them as dependencies on each other" vs "land commits" vs "put this into the pipeline itself".
- Must support optional context info.
- Use `lf pipeline` to run the inner loop.
- Resolve prompt/context relative to repo root by default.
- PR chaining should follow a Graphite-like UX to make chaining easy for humans.
- Worktrees are transient; allow multiple per background agent (worktree=branch).

## Done when

- A background agent can be registered via Maestro API with a prompt file + pipeline.
- Agent run includes prompt in all LLM requests for pipeline steps.
- Both outer loop strategies exist: PR chain and land commits.

## Quotes

> "maestro should have an API to register background agents."

> "Background agents get a few things:
> - a prompt file which define the agent's overall goals and is included in all the llm requests.
> - a pipeline which is a sequence of commands (like design -> implement -> review -> iterate -> expand -> polish etc) which is the "inner loop" of the agent.
> - maybe some context info
> - some sort of config on how to to handle the outerloop.  for starters we would Have maybe:
>  create PRs and chain them as dependencies on each other
> or
>   land commits
>
> this is tbd, maybe this could be put into the pipeline itself."

> "The goal is that I would be able to assign set up general areas of responsibility (maestro UI, background agents, onboarding and documentation quality, etc) and set the llms loose on those"

> "table in maestro sql i think -- they are personal state not codebase state for now"

> "agent loops are mostly via cli, will eventualyl be through swift maestro UI"

> "yes, we can use lf pipeline"

> "i want support for the two options for outer loop for now"

> "Dont know what you mean, but generally most things hould work the same (just always use repo root as base). PR chaining -- we should learn from how graphite tries to make chaining easy for humans."

> "Worktrees --i think worktree=branch so they should be transient and we should have potentially multiple per background agent"

## Open questions

- CLI shape: new `lf agent` command, or `lf maestro` subcommands?
- How should agent prompt file be resolved (repo-relative vs absolute path)?
- How should PR chaining be represented (branch naming, base branch selection)?
- Should agent loop runs create worktrees per loop or reuse one?
