# Heart Flows

Make writing and using flows fun.

## What to build

1. **DAG-based flows** — Steps form a directed acyclic graph, not a list
2. **Built-in flows** — Ship flows that encode best practices out of the box
3. **Fork/Synthesize** — Parallel branches that join naturally in the DAG

## Research: Workflow Orchestration Patterns

Studied: [Prefect](https://www.prefect.io/), [Hamilton](https://hamilton.apache.org/), [Dagster](https://dagster.io), Airflow, Temporal.

### Key Lessons

**Prefect 2/3 removed rigid DAGs.** Real workflows don't fit pre-planned graphs. They embraced native Python control flow—if/else, loops, dynamic task spawning. Dependencies are implicit via data flow: task B uses result of task A = B depends on A.

**Hamilton's elegance: deps via parameter names.** Function `B(A: int)` automatically depends on function `A()`. Zero boilerplate. But requires naming discipline.

**Airflow's pain points:**
- Top-level code parsed every 30 seconds (performance killer)
- Heavy infrastructure (scheduler, executor, metadata DB)
- Cross-DAG deps → monolithic mess
- Testing is "extremely difficult"
- Steep learning curve with Airflow-specific concepts

**Dagster middle ground:** Explicit `deps=[upstream]` or implicit via params. Asset-centric rather than task-centric.

### Our Domain is Different

Data pipeline tools pass data between steps. Our steps share a **git worktree**:
- "Dependencies" mean ordering, not data flow
- Steps mutate shared state (files in worktree)
- Fork/Synthesize is parallel *exploration*, not parallel *processing*
- Most real flows are linear

### Design Implications

1. **Don't force rigid DAGs.** Most flows are linear chains. Don't make users declare graphs for simple cases.
2. **Implicit ordering by default.** Steps run in declared order unless specified otherwise.
3. **DAG when needed.** Explicit deps for parallel branches: `after=["step-a", "step-b"]`
4. **Fork/Synthesize as primitives.** Not just "parallel nodes"—actual parallel agent exploration with synthesis.

## Flow as DAG

Flows are directed acyclic graphs. Simple cases use implicit linear ordering. Complex cases use explicit dependencies.

### Linear Flows (Common Case)

Most flows are chains. Keep them simple:

```python
def flow():
    return Flow("design", "implement", "polish")
```

Steps run in order. No graph declaration needed.

### Parallel Branches (When Needed)

Use `after` for explicit dependencies:

```python
def flow():
    return Flow(
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
        Step("polish", after="integrate"),
    )
```

```
design ──┬──> impl-api ──┬──> integrate ──> polish
         └──> impl-ui ───┘
```

### Step Configuration

Each step can override model, voice:

```python
Flow(
    Step("design", model="claude"),
    Step("implement", model="codex", after="design"),
    Step("review", model="claude", voice="critic", after="implement"),
)
```

### Execution Model

1. Build DAG from steps (linear if no `after` specified)
2. Topological sort
3. Execute in parallel where deps allow
4. Sequential steps share worktree; parallel steps get branched worktrees

## Built-in Flows

Ship these as defaults so users get value without writing Python:

| Flow | Steps | Use case |
|------|-------|----------|
| `ship` | design → implement → polish | Full feature workflow |
| `quick` | implement → polish | Fast iteration, skip design |
| `iterate` | review → implement → polish | Improve existing code |
| `reduce` | reduce → polish | Simplify bloated code |
| `roadmap` | roadmap → design | Strategic planning |

These live in `loopflow/flows/` (shipped with package) and are available everywhere.

## Fork/Synthesize

### The Pattern

Fork spawns N agents in parallel. Each runs independently in its own worktree. Synthesize reviews all outputs and produces a unified result.

```python
def flow():
    return Flow(
        "design",
        Fork(
            {"step": "implement", "voice": "architect"},
            {"step": "implement", "voice": "pragmatist"},
            {"step": "implement", "model": "codex"},
        ),
        Synthesize(),
        "polish",
    )
```

In the DAG, Fork creates parallel branches that Synthesize joins:

```
design ──> Fork ──┬──> impl (architect)  ──┐
                  ├──> impl (pragmatist) ──┼──> Synthesize ──> polish
                  └──> impl (codex)      ──┘
```

### Fork Semantics

Each fork item is a full agent config:

```python
@dataclass
class ForkAgent:
    step: str | None = None      # single step
    flow: str | None = None      # or full flow
    voice: str | None = None
    model: str | None = None
    area: str | None = None      # defaults to parent's area
```

Constraints:
- Must specify either `step` or `flow`
- Stimulus is always ONCE (run to completion)
- Each fork gets its own worktree branched from current state

### Synthesize Semantics

Synthesize receives context about all fork outputs:
- List of worktree paths
- Git diff from each (changes made by that fork)
- Fork config (which voice/model/etc.)

**Default behavior:**

1. **Analyze variation** — Document how the forks differed in approach, structure, tradeoffs
2. **Write synthesis notes** — Output to `scratch/synthesis.md` explaining the differences
3. **Produce unified result** — Pick the best approach or combine elements into the main worktree

The analysis is the point. Even if one fork is clearly better, understanding *why* the approaches diverged is valuable signal.

**Built-in synthesizer prompt (sketch):**

```markdown
You have N implementations of the same task from differently-configured agents.

For each fork:
- Summarize the approach taken
- Note key structural decisions
- Identify tradeoffs (performance, readability, flexibility)

Then:
- Document where they agreed (probably correct)
- Document where they diverged (interesting decision points)
- Explain which approach you're taking and why
- Write the unified implementation

Output your analysis to scratch/synthesis.md before writing code.
```

**Custom synthesizer:**

```python
Synthesize(prompt="Focus on performance tradeoffs")
Synthesize(step="my-custom-synthesizer")
```

### Execution Model

```
[current worktree @ commit A]
        │
        ├── Fork ──┬── worktree-1 (architect) → commits B1
        │          ├── worktree-2 (pragmatist) → commits B2
        │          └── worktree-3 (codex) → commits B3
        │
        ▼
[Synthesize reads B1, B2, B3 diffs against A]
[Writes new code to current worktree]
[Commits as C — the only commit that persists]
        │
        ▼
[polish continues from C]
```

**Fork worktrees are ephemeral.** They exist only for exploration. Their commits (B1, B2, B3) never merge—only the synthesizer's commit (C) appears in git history. After synthesis completes, fork worktrees are deleted.

This keeps history clean: you see "implement feature" not "merge 3 experimental branches."

Optional `--keep-forks` flag for debugging, but default is delete.

## Data Structures

```python
@dataclass
class Step:
    """A step with optional overrides and dependencies."""
    name: str
    after: str | list[str] | None = None  # None = follows previous step
    model: str | None = None
    voice: str | None = None

@dataclass
class ForkAgent:
    """Configuration for one agent in a Fork."""
    step: str | None = None      # single step
    flow: str | None = None      # or full flow
    voice: str | None = None
    model: str | None = None
    area: str | None = None      # defaults to parent's area

@dataclass
class Fork:
    """Spawn parallel agents."""
    agents: list[ForkAgent]
    # Max 5 agents for v1

    def __init__(self, *agents):
        self.agents = [ForkAgent(**a) if isinstance(a, dict) else a for a in agents]

@dataclass
class Synthesize:
    """Join fork results into unified output."""
    step: str | None = None     # custom synthesizer step
    prompt: str | None = None   # inline prompt override
    # If neither: use built-in synthesizer

@dataclass
class Flow:
    """DAG of steps, forks, and synthesizes."""
    steps: list[str | Step | Fork | Synthesize]

    def __init__(self, *steps):
        self.steps = [self._parse(s) for s in steps]

    def _parse(self, s):
        if isinstance(s, str):
            return Step(name=s)
        if isinstance(s, dict):
            return Step(**s)
        return s
```

**Parsing rules:**
- `"implement"` → `Step(name="implement")`
- `{"step": "implement", "model": "codex"}` → `Step(name="implement", model="codex")`
- `Step(...)`, `Fork(...)`, `Synthesize(...)` pass through

## Key Functions

```python
def run_fork(fork: Fork, base_commit: str, parent_worktree: Path) -> list[ForkResult]:
    """Create worktrees from base_commit, run each agent in parallel, return results."""

def run_synthesize(
    synth: Synthesize,
    fork_results: list[ForkResult],
    base_commit: str,
    target_worktree: Path,
) -> None:
    """Review fork diffs against base, write unified result + analysis to target."""

def cleanup_fork_worktrees(results: list[ForkResult]) -> None:
    """Delete temporary fork worktrees after synthesis."""
```

```python
@dataclass
class ForkResult:
    worktree: Path
    config: ForkAgent
    diff: str           # git diff base_commit..HEAD
    status: str         # completed, failed, timeout
    scratch_notes: str  # contents of scratch/ if any
```

**Synthesizer context assembly:**

The synthesizer step receives a structured context block:

```markdown
## Fork Results

### Fork 1: architect voice
Config: step=implement, voice=architect, model=claude

<diff>
... changes made by this fork ...
</diff>

### Fork 2: pragmatist voice
Config: step=implement, voice=pragmatist, model=claude

<diff>
... changes made by this fork ...
</diff>

### Fork 3: codex model
Config: step=implement, model=codex

<diff>
... changes made by this fork ...
</diff>
```

## Constraints

- Fork must be followed by Synthesize (or flow ends with dangling worktrees)
- Can't nest Fork inside Fork (for now)
- All fork agents share the same starting point (current worktree state)

## Done When

```bash
# Built-in flows work
lf flow ship
lf flow quick
lf flow iterate

# Parallel branches work
cat > .lf/flows/parallel.py << 'EOF'
def flow():
    return Flow(
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
    )
EOF

lf flow parallel: add dashboard
# design runs, then impl-api and impl-ui in parallel, then integrate

# Fork/Synthesize works
cat > .lf/flows/race.py << 'EOF'
def flow():
    return Flow(
        Fork(
            {"step": "implement", "voice": "architect"},
            {"step": "implement", "voice": "pragmatist"},
        ),
        Synthesize(),
    )
EOF

lf flow race: add caching
# Creates 2 worktrees, runs implement in each, synthesizes result
```

## UI Changes (Concerto)

Fork/Synthesize introduces parallel execution. Concerto needs:

1. **Fork progress view** — Show N agents running in parallel with individual status
2. **Synthesis preview** — Display `scratch/synthesis.md` analysis before/after
3. **Worktree indicators** — Visual distinction for temporary fork worktrees

Minimal: CLI-only for v1. Concerto support in follow-up.

## Code Architecture

### Current Structure

```
src/loopflow/
├── lf/
│   ├── flows.py      # Flow data structures, loading, resolution
│   ├── flow.py       # Flow execution (runs steps, handles fork/join)
│   ├── step.py       # Step file loading
│   └── builtins/     # Built-in steps
└── lfd/
    └── execution/    # Agent execution (collector, runner, worker)
```

### Changes Needed

**`lf/flows.py`** — Add new data structures:
- `Step` with `after` for explicit dependencies (rename existing `FlowStep`)
- `Fork` (parallel agent spawn) — replaces current `fork` field on FlowStep
- `Synthesize` (join fork results) — extends current `Join`
- Keep `Flow`, `Choose` as-is

**`lf/flow.py`** — Extend execution:
- Add DAG builder from step list (infer linear ordering when no `after`)
- Add topological sort
- Extend `_run_fork_join_group` for new Fork/Synthesize semantics
- Add synthesis analysis output to `scratch/synthesis.md`

**`lf/builtins/flows/`** — New directory for built-in flows:
```
src/loopflow/lf/builtins/flows/
├── __init__.py
├── ship.py
├── quick.py
├── iterate.py
├── reduce.py
└── README.md
```

Update `load_flow()` in `flows.py` to check builtins after repo/global.

### Folder READMEs

**`.lf/flows/README.md`** (user-facing, created by `lf init`):
```markdown
# Flows

Define flows as Python files. Each flow returns a `Flow` with steps.

## Example

```python
# .lf/flows/ship.py
def flow():
    return Flow("design", "implement", "polish")
```

Run with `lf flow ship`.

## Parallel Branches

```python
def flow():
    return Flow(
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
    )
```

## Fork/Synthesize

```python
def flow():
    return Flow(
        Fork(
            {"step": "implement", "voice": "architect"},
            {"step": "implement", "model": "codex"},
        ),
        Synthesize(),
    )
```

See docs for more: https://loopflow.dev/docs/flows
```

**`src/loopflow/lf/builtins/flows/README.md`** (developer-facing):
```markdown
# Built-in Flows

Flows shipped with loopflow. Available everywhere without user configuration.

| Flow | Steps | Use case |
|------|-------|----------|
| ship | design → implement → polish | Full feature workflow |
| quick | implement → polish | Fast iteration |
| iterate | review → implement → polish | Improve existing code |
| reduce | reduce → polish | Simplify bloated code |
| roadmap | roadmap → design | Strategic planning |

## Adding a Built-in Flow

1. Create `{name}.py` with a `flow()` function
2. Return a `Flow` with steps
3. Update this README

Fork failure handling is undefined for v1. Document edge cases here as they're discovered.
```

## Implementation Path

**Phase 1: DAG execution model**
- Refactor `FlowStep` → `Step` with `after` field
- Add DAG builder to `flows.py`
- Implement topological sort in `flow.py`
- Parallel branches get branched worktrees

**Phase 2: Built-in flows**
- Create `src/loopflow/lf/builtins/flows/`
- Add ship, quick, iterate, reduce, roadmap
- Update `load_flow()` to check builtins
- Generate `.lf/flows/README.md` in `lf init`

**Phase 3: Fork/Synthesize**
- Add `Fork` and `Synthesize` to `flows.py`
- Extend `_run_fork_join_group` for new semantics
- Add synthesis analysis to `scratch/synthesis.md`
- Write built-in synthesizer step

## Use Cases: Model Experimentation

### Race Models

Fork the same task across models, see which approach wins:

```python
def flow():
    return Flow(
        Fork(
            {"step": "implement", "model": "claude"},
            {"step": "implement", "model": "codex"},
            {"step": "implement", "model": "gemini"},
        ),
        Synthesize(),
    )
```

The synthesis analysis documents how each model approached the problem—valuable signal for understanding model strengths.

### Hybrid Model Flows

Different models excel at different tasks. Claude for judgment and design, Codex for refactoring:

```python
def flow():
    return Flow(
        Step("design", model="claude"),
        Step("implement", model="codex"),
        Step("review", model="claude"),
        Step("polish", model="claude"),
    )
```

### Voice × Model Matrix

Combine voice and model variation for maximum exploration:

```python
def flow():
    return Flow(
        Fork(
            {"step": "implement", "voice": "architect", "model": "claude"},
            {"step": "implement", "voice": "architect", "model": "codex"},
            {"step": "implement", "voice": "pragmatist", "model": "claude"},
            {"step": "implement", "voice": "pragmatist", "model": "codex"},
        ),
        Synthesize(),
    )
```

Four approaches: architect-claude, architect-codex, pragmatist-claude, pragmatist-codex. Synthesizer picks the best.

## Documentation Strategy

Update README and docs to showcase flows as the power feature, not just "step chaining."

### README Changes

Lead with the exciting stuff. Current README shows steps first, flows second. Flip it:

```markdown
## Flows

Run a complete workflow with one command:

```bash
lf flow ship: add user auth
```

`ship` runs design → implement → polish, committing between each step.

### Built-in Flows

| Flow | What it does |
|------|--------------|
| `ship` | Design, build, polish — full feature workflow |
| `quick` | Build and polish — skip design for small changes |
| `iterate` | Review, fix, polish — improve existing code |
| `race` | Try multiple approaches, pick the best |

### Race Different Models

```bash
lf flow race: add caching
```

Runs the same task with Claude, Codex, and different voices in parallel. Synthesizes the best approach. You get the winning implementation *and* a `scratch/synthesis.md` explaining what each tried.
```

### docs/index.md Changes

Add a "Why Flows?" section early:

```markdown
## Why Flows?

Steps are atomic. Flows are how work actually gets done.

**Linear flows** chain steps with automatic commits:
```
design → implement → polish
```

**Parallel flows** branch and join:
```
design ──┬──> impl-api ──┬──> integrate
         └──> impl-ui ───┘
```

**Fork/Synthesize** explores multiple approaches:
```
Fork ──┬──> impl (architect)  ──┐
       ├──> impl (pragmatist) ──┼──> Synthesize
       └──> impl (codex)      ──┘
```

The synthesizer doesn't just pick a winner—it documents *why* approaches differed.
```

### Key Messaging

1. **Flows are the product.** Steps are building blocks. Flows are what users run.

2. **Built-ins are good defaults.** Users don't need to write flows to get value. `lf flow ship` just works.

3. **Fork/Synthesize is the differentiator.** No other tool does parallel agent exploration with synthesis analysis. This is the "wow" feature.

4. **Model racing is concrete.** "Race Claude vs Codex" is immediately understandable and compelling.

5. **Analysis is the point.** The synthesis doc isn't just a side effect—it's how you learn which model/voice works best for your codebase.

### Example-First Documentation

Every flow concept gets a runnable example before explanation:

```markdown
## Parallel Branches

```bash
lf flow parallel: add dashboard
```

Where `parallel.py` is:

```python
def flow():
    return Flow(
        Step("design"),
        Step("impl-api", after="design"),
        Step("impl-ui", after="design"),
        Step("integrate", after=["impl-api", "impl-ui"]),
    )
```

API and UI implementation run in parallel, then integrate.
```

## Decisions

**Fork failure handling:** Defer to follow-up. Document in `loopflow/flows/README.md` that partial failure behavior is undefined for v1.

**Worktree naming:** Use existing agent naming scheme. Fork worktrees get names like `repo.fork-<flow>-<n>` following the `{repo}.{name}` pattern.

**Resource limits:** Hard limit of 5 parallel forks for v1. Error if Fork has more than 5 agents. Can revisit with config override later.
