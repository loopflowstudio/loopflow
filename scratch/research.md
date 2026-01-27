# Research: Loopflow Codebase

## System understanding

Loopflow orchestrates waves of autonomous work using LLM coding agents (Claude Code, Codex, Gemini CLI). The architecture centers on **steps** (atomic prompts) that chain into **flows** (DAGs) and scale into **waves** (persistent autonomous units).

### Architecture

Three CLIs with distinct responsibilities:

| Module | Purpose | Entry |
|--------|---------|-------|
| `lf` | Prompt launcher—assembles context, runs steps | `src/loopflow/lf/cli.py` |
| `lfd` | Wave orchestration daemon—manages persistent autonomous work | `src/loopflow/lfd/cli.py` |
| `lfops` | Git workflow helpers—worktrees, commits, PRs | `src/loopflow/lfops/commands.py` |

```
lf/                     (~8000 LOC)
├── cli.py              # Typer CLI entry point
├── step.py             # Step execution (run, inline, flow)
├── flow.py             # Flow DAG execution
├── flows.py            # Flow/Step/Fork parsing
├── context.py          # Prompt assembly, token management
├── config.py           # Config loading & merging
├── directions.py       # Direction file loading
├── frontmatter.py      # YAML frontmatter parsing
├── design.py           # Design artifact helpers
├── files.py            # File library gathering
├── launcher.py         # Runner selection (claude/codex/gemini)
├── skills.py           # External skill discovery
└── builtins/           # Steps, flows, directions
    ├── steps/
    │   ├── plan/       # review, reduce, expand, polish, etc.
    │   ├── code/       # debug, implement, compress, gate
    │   ├── ops/        # consolidate, add-to-roadmap, synthesize
    │   └── interactive/
    ├── flows/
    │   ├── code/       # ship, grind, incident
    │   └── plan/       # roadmap-reduce, roadmap-polish
    └── directions/
        ├── roles/      # designer, product-engineer, ceo
        └── values/     # scale, craft, flow

lfd/                    (~4000 LOC)
├── cli.py              # lfd create, loop, watch, cron
├── models.py           # Wave, FlowRun, StepRun, Stimulus
├── wave.py             # Wave persistence, stimulus checking
├── db.py               # SQLite connection & migrations
├── step_run.py         # Step execution logging
├── daemon/
│   ├── server.py       # Asyncio socket server
│   ├── protocol.py     # JSON-RPC format
│   ├── manager.py      # Wave manager
│   └── client.py       # Client for daemon communication
└── execution/
    ├── runner.py       # run_iteration() entry point
    └── collector.py    # Output capture, JSON streaming

lfops/
├── commands.py         # Command registration
├── commit.py           # Commit message generation
├── pr.py               # PR creation/management
├── rebase.py           # Rebase operations
├── land.py             # Land-to-main workflow
└── worktree/           # Worktree management
```

### Data flow

**Step execution path:**
```
lf <step> → gather_prompt_components() → format_prompt() → launcher → agent CLI
```

**Wave execution path:**
```
lfd loop <wave> → daemon scheduler (5s poll) → run_iteration() → run_flow() → step sequence
```

**Context assembly:**
1. Gather docs (scratch/, roadmap/, *.md)
2. Get branch diff/files
3. Load step file
4. Resolve directions
5. Add clipboard/summaries if requested
6. Trim to token limits (~200k total, with per-section budgets)

### Key abstractions

**Step**: Markdown file with YAML frontmatter that tells an agent what to do. Steps are atomic—each does one thing and produces artifacts for the next.

```python
@dataclass
class StepConfig:
    interactive: bool | None         # Run mode override
    include: list[str]               # File patterns to include
    exclude: list[str]               # File patterns to exclude
    model: str | None                # Model override
    direction: list[str] | None      # Direction files
    area: str | None                 # Area scope
    chrome: bool | None              # Browser automation
```

**Flow**: Chains steps with commits between. Supports Fork (parallel) and Choose (conditional).

```python
class Flow:
    steps: list[Step | Fork | Choose]

@dataclass
class Step:
    name: str
    after: str | list[str]       # Dependencies (or None = sequential)
    direction: str | None        # Override
    interactive: bool | None     # Override
    model: str | None            # Override

@dataclass
class Fork:
    step: str                    # Step to run N times
    directions: list[str]        # Different directions per branch
    synthesize: bool             # Whether to synthesize results
```

**Wave**: Persistent autonomous unit. `area × direction × flow × stimulus`.

```python
class Wave:
    name: str
    area: list[str]              # Paths to work on
    direction: list[str]         # Judgment/intent
    flow: str                    # Step sequence to run
    stimulus: Stimulus           # once, loop, watch, cron
    worktree: Path               # Persistent worktree
    status: WaveStatus           # idle, running, stopped
```

**Direction**: Shapes agent judgment. Markdown files in `.lf/directions/` or builtins. Loaded and injected as XML sections in the prompt.

**PromptComponents**: Everything that gets assembled into the prompt.
```python
@dataclass
class PromptComponents:
    docs: list[tuple[Path, str]]     # README, roadmap/, scratch/
    diff_files: list[...]            # Full content of changed files
    step: tuple[str, str]            # (filename, content)
    direction: list[Direction]       # Judgment/style
    clipboard: ClipboardContent | None
    images: list[Path]               # Visual context
```

**Config**: Layered configuration with global → repo → CLI precedence.

```python
@dataclass
class Config:
    agent_model: str = "claude:opus"
    context: list[str] = []          # Default files to include
    exclude: list[str] = []          # Patterns to skip
    direction: list[str] | None = None
    area: str | None = None
    budgets: BudgetConfig            # Token limits per section
    lfdocs: bool = True              # Include repo docs
    diff_files: bool = True          # Include branch changes
    yolo: bool = False               # Skip permissions
```

## Tensions

**Interactive vs auto modes.** `lf/step.py` handles both, but interactive uses `os.execvp()` (replaces process) while flow execution uses `subprocess.run()` (continues after). The flow path at `flow.py` needed special handling for interactive steps within flows—they use subprocess so the flow can continue.

**Worktree ownership.** Waves use persistent worktrees (`wave.worktree`) but fork execution creates ephemeral worktrees. The two models have different lifecycles—wave worktrees survive across iterations, fork worktrees are cleaned up after synthesis.

**Token trimming vs context completeness.** `lf/context.py` drops components greedily (largest first) to stay under budget, but this can remove the very files the agent needs. For a `debug` step, the diff is often most important but also largest.

**Config layering complexity.** Some settings override (scalars), others combine (lists like `context`, `exclude`). The merging logic in `config.py` handles this, but it's not obvious from reading the config files which behavior applies.

## Observations

### Complexity

**`lf/context.py` (718 lines)**: The densest file. `gather_prompt_components()` has ~15 parameters and orchestrates 8+ helper functions. Token budgeting logic is spread across multiple functions (`trim_prompt_components`, `analyze_components`, `_drop_candidates`). The XML formatting and component assembly are interleaved.

**`lf/flow.py` (540 lines)**: Flow execution handles Steps, Forks, and Choose in a single loop. Fork parallelism (`run_fork()`, `run_synthesize()`) adds ~150 lines. The topological batch execution for parallel steps adds complexity but enables DAG execution.

**`lf/config.py` (300 lines)**: Config merging has special cases for additive keys. The `_merge_configs()` function handles scalars differently than lists, and there are comments explaining the edge cases.

**`lfd/daemon/server.py` (400 lines)**: Asyncio server with method dispatch. Protocol is JSON-over-newline but this is only documented in `protocol.py`. Periodic checks (`_periodic_check()`) poll every 5s for wave triggers.

### Quality

**Strong typing throughout.** Pydantic models for config, dataclasses for internal state. Type hints on all public functions. The codebase follows its own style guide.

**Consistent naming.** Verb-first functions (`find_prompt`, `load_config`, `gather_area`), underscore prefix for private (`_run_step`, `_execute_step`), clean separation between parsing and execution.

**Documentation quality varies:**
- `CLAUDE.md`, `PROMPT_STYLE.md`: Excellent, comprehensive
- Inline comments: Sparse but present where needed
- Docstrings: Mostly one-line or skipped per style guide
- Fork/synthesize algorithm: Not explained in code comments
- Token trimming priority: Why largest first? Not documented

**Error handling follows the guide.** Return `None` for "not found", raise exceptions for "shouldn't happen". Error messages include context (`f"Config file not found at {path}"`).

**Test coverage:**
- `tests/test_context.py` (24KB): Thorough context assembly tests
- `tests/test_flows.py`: DAG building, parsing
- `tests/test_lfd.py`: Wave DB, stimulus logic
- Gap: Fork/synthesize execution lightly tested
- Gap: Interactive step handling within flows

### Potential

**Choose step exists but is underused.** Defined in `flows.py` and `flow.py` has `choose_branch()`, but no builtin flows use it. Could enable conditional workflows (if tests pass → ship, else → debug).

**Tick-based resumption.** `FlowRun.step_index` exists but isn't used for resumption after failures. Could enable "resume from step 3" functionality if a flow fails mid-way.

**Event streaming foundation.** `lfd/daemon/protocol.py` defines Event type and server has subscribe method. Could power real-time UI without polling if fully implemented.

**Parallel step batches.** `topological_batches()` already groups independent steps. Currently serializes within batches but could run truly parallel with process isolation.

**Skills infrastructure is extensible.** `skills.py` already supports multiple sources (superpowers, local paths, SkillRegistry). Adding new skill providers would be straightforward.

## Open questions

- The `lfd/execution/worker.py` file exists but `runner.py` handles execution. Is worker.py unused or planned for future parallelism?

- Token budgets are hard-coded (50k area, 30k docs, 20k diff). Should these be configurable per step or flow?

- `FlowRun.step_index` tracks position but nothing uses it for resumption. Dead code or planned feature?

- Why does `context.py` drop diff_files first when trimming? For a debug step, the diff is crucial context.

## Recommendations

### Document token trimming strategy

**Observation**: `context.py` trims components without explaining the priority. The algorithm drops largest components first, but the rationale isn't documented.

**Cost**: Low—add a comment block explaining the priority and rationale.

**Benefit**: Maintainers understand why components drop in a particular order. Users can predict behavior.

**Verdict**: Worth doing. A 10-line comment would clarify the design decision.

### Remove or document worker.py

**Observation**: `lfd/execution/worker.py` exists but `runner.py` handles execution. The file may be unused or planned for future parallelism.

**Cost**: Low—either delete or add a docstring explaining its purpose.

**Benefit**: Reduces confusion about execution architecture.

**Verdict**: Worth investigating. `grep` for imports/usages first. If unused, delete. If planned, document.

### Add fork/synthesize tests

**Observation**: `tests/test_flows.py` tests Flow parsing but fork execution is untested. `flow.py:run_fork()` and `run_synthesize()` have complex worktree logic.

**Cost**: Medium—need to mock worktree creation and parallel execution.

**Benefit**: Fork is a key feature (roadmap flows use it). Without tests, regressions go unnoticed.

**Verdict**: Worth doing before adding more fork features.

### Consolidate context.py complexity

**Observation**: `gather_prompt_components()` has 15+ parameters and calls 8 helpers. Token trimming is spread across 3 functions. The file is the largest in `lf/` at 718 lines.

**Cost**: Medium—requires careful refactoring to preserve behavior.

**Benefit**: Easier to understand and maintain. Clearer separation between gathering, trimming, and formatting.

**Verdict**: Worth considering but not urgent. The code works; complexity is a readability issue, not a correctness issue.

### Make token budgets configurable

**Observation**: Budgets (50k area, 30k docs, 20k diff) are hard-coded. Different workflows might need different balances—debug steps want more diff, design steps want more docs.

**Cost**: Medium—add to Config, wire through gather/trim functions.

**Benefit**: Users can tune context assembly for their workflows.

**Verdict**: Nice to have. Current defaults work for most cases. Lower priority than documentation fixes.
