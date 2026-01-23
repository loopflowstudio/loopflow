# Simplification Opportunities

## Product intent

Loopflow is a tight, focused tool for prompt and context assembly. The core value: gather context, store prompts as markdown files, pass to coding agents. "Tight loops. Do one thing, hand off cleanly."

An agent is **flow × area × voice**—three multiplied concepts, not a complex hierarchy.

---

## Opportunity 1: Agent mode as discriminated union, not nullable columns

**Misalignment**: The product describes three distinct agent modes (Loop, Watch, Cron) as equals, but the architecture stores mode implicitly via nullable columns (`watch_paths`, `cron`) on a single Agent table.

**Symptom**: The `mode` property computes what should be explicit data:

```python
# models.py:98-104
@property
def mode(self) -> str:
    if self.watch_paths:
        return "watch"
    if self.cron:
        return "cron"
    return "loop"
```

The migration `m_2025_01_22_unified_agents.py` had to consolidate three separate tables (loops, subscriptions, schedules) into one by adding nullable columns. This suggests the original intuition was right—modes are distinct—but the consolidation went too far.

**Realignment**: Make mode explicit in the data model:

```python
class AgentMode(str, Enum):
    LOOP = "loop"
    WATCH = "watch"
    CRON = "cron"

class Agent(LfdModel):
    mode: AgentMode
    trigger_config: str | None  # watch_paths for WATCH, cron expression for CRON, None for LOOP
```

Or use three tables again with a shared base—but with explicit naming this time (`loop_agents`, `watch_agents`, `cron_agents`) rather than the previous generic names.

**Cascade**:
- CLI validation becomes simpler (mode is explicit input, not inferred)
- Status display doesn't need to compute mode
- Database queries can filter by mode directly
- TypeScript/Swift models become cleaner (no nullable-means-something pattern)

---

## Opportunity 2: Flows are lists, not DAGs

**Misalignment**: The product describes flows as "chains of steps"—sequential execution with commits between. The architecture has a full DAG system with `fork`, `join`, `Choose`, and parallel execution.

**Symptom**: Looking at actual flow definitions in the repo:

```python
# submit.py - simple list
def flow():
    return {"steps": ["polish", "draft_commit"]}

# ux.py - simple list
def flow():
    return {"steps": ["ux-research", "ux-gaps", "ux-fix"]}

# ship.py - uses Choose/fork/join
SHIP = Flow(
    Choose(
        options={
            "add_to_roadmap": Flow(fork/join machinery...),
            "scope_from_roadmap": Flow("design_from_roadmap", "implement", "polish"),
        }
    ),
)
```

Two of three flows are simple lists. The complex one (`ship.py`) looks experimental. Meanwhile, `flows.py` has 308 lines of DAG resolution code: `FlowStep`, `FlowDef`, `ResolvedStep`, `resolve_flow()`, `Choose`, `Join`, `JoinConfig`, parallel group tracking.

**Realignment**: A flow is a list of step names. Period.

```python
class FlowDef(BaseModel):
    name: str
    steps: list[str]

def load_flow(name: str, repo: Path) -> FlowDef | None:
    """Load flow from flows/{name}.py."""
    # ... load module, call flow(), expect list[str]
```

If parallel execution is ever needed, it's a different primitive (`lf fork`) not flow machinery. The Choose pattern is better handled by a conditional step prompt than flow-level branching.

**Cascade**:
- `flows.py` shrinks from 308 lines to ~50
- No `ResolvedStep`, no parallel group tracking
- Flow execution is a simple for-loop
- CLI can show flow as `step → step → step` directly
- TypeScript/Swift models drop `fork`, `join`, `choose` types

---

## Opportunity 3: `lfd run` creates unnecessary agents

**Misalignment**: The product distinguishes between one-shot execution (`lf`) and daemon-managed agents (`lfd`). But `lfd run` creates a "temporary agent" then runs it once:

```python
# lfd/cli.py:365-374
def run(...):
    # Create a temporary agent and run it once
    agent = create_agent(repo=repo, flow=flow_name, voice=voice_list, area=[area])
    result = start_agent(agent.id, foreground=True)
```

This conflates the concepts. An agent is a persistent entity that runs on triggers. A one-shot flow execution doesn't need an agent at all.

**Symptom**: After `lfd run ship .`, there's a zombie agent in the database with status=IDLE and iteration=1. It served its purpose and will never run again, but it's stored as if it might.

**Realignment**:
- `lfd run` → remove command entirely
- Use `lf flow ship` (or just `lf ship`) for one-shot flow execution
- `lfd` manages only persistent agents: `lfd loop`, `lfd subscribe`, `lfd schedule`

Or rename `lfd run` to something that doesn't imply creating an agent—maybe `lfd exec` or just have `lf flow` support the `--area` flag.

**Cascade**:
- Clear mental model: `lf` = run prompts, `lfd` = manage agents
- No orphan agents from one-shot runs
- Agent table only contains actual agents
- Status display is cleaner (only shows things that matter)

---

## Aligned areas (patterns to preserve)

**Context assembly is well-designed.** The `PromptComponents` dataclass, `ContextConfig`, and `gather_prompt_components()` are clean. Token trimming works. The display of context breakdown is useful. This is the core value prop and it works.

**Step resolution is clean.** The search order (repo → global → builtin) with clear precedence, frontmatter parsing, and `StepConfig` merging are solid. Steps as markdown files in `.claude/commands/` or `.lf/steps/` is the right abstraction.

**StepRun tracking is appropriate.** Logging individual step executions to the database makes sense—it's the unit of work that matters for debugging and history. The rename from `sessions` → `step_runs` was correct terminology.

**Voice as files, not config.** Voices are markdown files in `.lf/voices/`. This is the right abstraction—they're documents, not structured data. The validator-based parsing (`load_voice`) is appropriate.

---

## Summary

| Opportunity | Effort | Impact |
|-------------|--------|--------|
| Agent mode as discriminated union | S | Cleaner data model, simpler CLI |
| Flows are lists, not DAGs | M | Delete ~250 lines, simpler mental model |
| Remove `lfd run` | S | Clear separation of concerns |

The theme: the architecture has grown features that the product doesn't use. Fork/join flows, implicit agent modes, one-shot agents—all complexity that doesn't match how the tool is actually used. Realigning means deleting code, not adding it.
