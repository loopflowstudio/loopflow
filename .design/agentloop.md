# Agent Looping API

Agent loops cycle through a pipeline continuously, producing one PR per iteration.

## Review

**Verdict:** Needs work

### Missing `pipeline` command handling

The diff removes the `pipeline` command from `run.py:714-842` but the import at line 22 still references `RaceConfig` from `lf.pipelines`. The command itself is removed but there's no replacement registered. Looking at `__init__.py`, `pipeline` is removed from `known_commands` but there's no indication how users should run pipelines now.

Either restore the pipeline command or document the migration path.

### `goals.py` is duplicated in uncommitted changes

The diff shows `src/loopflow/lf/goals.py` as a new file (in uncommitted changes), but the runner already imports `from loopflow.lf.goals import load_goal`. This works because the file exists, but the design doc mentions this file as "new" when it's already implemented.

### Goal files diverge from design spec

The design doc shows goal files like:

```markdown
## Done when
Never—continuous improvement
```

But the actual goal files use different structure:
- `## Ultimate Goal` instead of just a one-line goal
- `## Quality Bar` instead of `## Done when`
- More verbose explanations

This is fine—the implementation is the source of truth—but the design doc should be updated to match or removed.

### Type inconsistency: `goal` field

`AgentSpec.goal` changed from `Path | None` to `str`. The design doc still shows `goal: Path | None`. This is a refinement (string is simpler), but the design doc is now stale.

### Missing imports in uncommitted files

Looking at `loop.py`, it imports from `loopflow.lf.goals` and `loopflow.lfd.db`, both of which have uncommitted changes. Need to commit `goals.py` before this works.

### Area scoping is incomplete

The design mentions area should restrict context gathering:

```python
def gather_prompt_components(..., area: list[str] | None = None):
    if area:
        diff_files = [f for f in diff_files if matches_area(f, area)]
```

But the implementation in `runner.py:110-113` just passes area paths as context:

```python
if agent.area:
    context_paths = list(agent.area)
```

This adds area paths to context but doesn't restrict the diff files. The behavior is additive, not restrictive as designed.

### `_handle_merge` return type change is good

The merge handlers now return `tuple[int, str | None]` to capture PR URLs. This is cleaner than the previous approach and enables PR tracking. No issues here.

## Design notes

### Goals as role-based personas

Goals are now role-based (designer, product-engineer, infra-engineer) rather than metric-based (80% test coverage). The role approach works better for continuous improvement where there's no measurable endpoint.

The goal file structure is:
- `## Ultimate Goal` — the north star
- `## Each Iteration` — what to do in each loop cycle
- `## Quality Bar` — definition of done per PR

### Area vs Goal distinction

- **Area**: WHERE the agent works (pathset, structured)
- **Goal**: WHAT the agent does (prose, referenced by name)

The area is used for context scoping; the goal is injected as a prompt prefix.

### Event broadcasting for observability

New events added for UI updates:
- `loop.step.started`
- `loop.step.completed`
- `loop.iteration.done`

These enable Maestro to show real-time step progress without polling.
