# Heart Flows

Make writing and using flows fun.

## What to build

1. **Built-in flows** — Ship flows that encode best practices out of the box
2. **Fork/Synthesize** — First dynamic flow construct: spawn N agents, collect outputs, synthesize

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
    return Flow([
        "design",
        Fork([
            {"step": "implement", "voice": "architect"},
            {"step": "implement", "voice": "pragmatist"},
            {"step": "implement", "model": "codex"},
        ]),
        Synthesize(),
        "polish",
    ])
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
class FlowStep:
    """A step with optional overrides."""
    step: str
    model: str | None = None
    voice: str | None = None

@dataclass
class Fork:
    agents: list[ForkAgent]
    # Always parallel, limit 5

@dataclass
class Synthesize:
    step: str | None = None     # custom synthesizer step
    prompt: str | None = None   # inline prompt
    # If neither: use built-in synthesizer
```

Flow steps can be strings or dicts:

```python
@dataclass
class Flow:
    steps: list[str | dict | FlowStep | Fork | Synthesize]
```

When parsing:
- `"implement"` → FlowStep(step="implement")
- `{"step": "implement", "model": "codex"}` → FlowStep(step="implement", model="codex")

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

# Fork/Synthesize works
cat > .lf/flows/race.py << 'EOF'
def flow():
    return Flow([
        Fork([
            {"step": "implement", "voice": "architect"},
            {"step": "implement", "voice": "pragmatist"},
        ]),
        Synthesize(),
    ])
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

## Implementation Path

**Phase 1: Built-in flows**
- Add `loopflow/flows/` with ship, quick, iterate, reduce
- Update flow loading to check built-ins after repo flows
- No new primitives, just shipped content

**Phase 2: Fork/Synthesize**
- Add `Fork` and `Synthesize` to flow DSL
- Implement `run_fork()` — parallel worktree creation + agent execution
- Implement `run_synthesize()` — context assembly + built-in prompt
- Wire into flow runner

## Use Cases: Model Experimentation

### Race Models

Fork the same task across models, see which approach wins:

```python
def flow():
    return Flow([
        Fork([
            {"step": "implement", "model": "claude"},
            {"step": "implement", "model": "codex"},
            {"step": "implement", "model": "gemini"},
        ]),
        Synthesize(),
    ])
```

The synthesis analysis documents how each model approached the problem—valuable signal for understanding model strengths.

### Hybrid Model Flows

Different models excel at different tasks. Claude for judgment and design, Codex for refactoring:

```python
def flow():
    return Flow([
        {"step": "design", "model": "claude"},
        {"step": "implement", "model": "codex"},
        {"step": "review", "model": "claude"},
        {"step": "polish", "model": "claude"},
    ])
```

This requires steps to accept per-step model overrides. Flow steps become either:
- `str` — step name, uses flow default model
- `dict` — step config with optional `model`, `voice`, etc.

```python
@dataclass
class FlowStep:
    step: str
    model: str | None = None
    voice: str | None = None
```

### Voice × Model Matrix

Combine voice and model variation for maximum exploration:

```python
Fork([
    {"step": "implement", "voice": "architect", "model": "claude"},
    {"step": "implement", "voice": "architect", "model": "codex"},
    {"step": "implement", "voice": "pragmatist", "model": "claude"},
    {"step": "implement", "voice": "pragmatist", "model": "codex"},
])
```

Four approaches: architect-claude, architect-codex, pragmatist-claude, pragmatist-codex. Synthesizer picks the best.

## Decisions

**Fork failure handling:** Defer to follow-up. Document in `loopflow/flows/README.md` that partial failure behavior is undefined for v1.

**Worktree naming:** Use existing agent naming scheme. Fork worktrees get names like `repo.fork-<flow>-<n>` following the `{repo}.{name}` pattern.

**Resource limits:** Hard limit of 5 parallel forks for v1. Error if Fork has more than 5 agents. Can revisit with config override later.
