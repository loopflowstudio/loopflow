# Research: Loopflow Codebase

## System understanding

Loopflow orchestrates waves of autonomous work using LLM coding agents (Claude Code, Codex, Gemini CLI). The architecture centers on **Waves**—persistent configurations that execute **Flows** (step sequences) in various stimulus modes.

### Architecture

Three CLIs with distinct responsibilities:

| Module | Purpose | Entry |
|--------|---------|-------|
| `lf` | Prompt launcher—assembles context, runs steps | `src/loopflow/lf/cli.py` |
| `lfd` | Wave orchestration daemon—manages persistent autonomous work | `src/loopflow/lfd/cli.py` |
| `lfops` | Git workflow helpers—worktrees, commits, PRs | `src/loopflow/lfops/commands.py` |

```
lf/
├── cli.py          # lf run, lf flow, lf inline
├── step.py         # Step execution (interactive/auto)
├── flow.py         # Flow orchestration, fork/synthesize
├── flows.py        # Flow/Step/Fork parsing
├── context.py      # Prompt assembly, token management
├── launcher.py     # Runner selection (claude/codex/gemini)
├── config.py       # Config loading & merging
└── builtins/       # Steps, flows, directions

lfd/
├── cli.py          # lfd create, loop, watch, cron
├── models.py       # Wave, FlowRun, StepRun, Stimulus
├── wave.py         # Wave persistence, stimulus checking
├── daemon/
│   ├── server.py   # Asyncio socket server
│   └── manager.py  # Concurrency control
└── execution/
    ├── runner.py   # run_iteration() entry point
    └── collector.py # Output capture, JSON streaming
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
6. Trim to token limits (~200k)

### Key abstractions

**Wave**: Persistent autonomous unit. `area × direction × flow × stimulus`.

```python
class Wave:
    area: list[str]              # Paths to work on
    direction: list[str]         # Judgment/intent (product-engineer, designer)
    flow: str                    # Step sequence to run
    stimulus: Stimulus           # once, loop, watch, cron
    worktree: Path               # Persistent worktree
```

**Flow**: Chains steps with commits between. Supports Fork (parallel) and Choose (conditional).

```python
class Flow:
    steps: list[Step | Fork | Choose]

class Step:
    name: str
    after: str | list[str]       # Dependencies (or None = sequential)
    direction: str | None        # Override
    interactive: bool | None     # Override
```

**Direction**: Shapes agent judgment. Markdown files in `.lf/directions/` or builtins.

**PromptComponents**: Everything that gets assembled into the prompt.
```python
@dataclass
class PromptComponents:
    docs: list[tuple[Path, str]]     # README, roadmap/, scratch/
    diff_files: list[...]            # Full content of changed files
    step: tuple[str, str]            # (filename, content)
    direction: list[Direction]       # Judgment/style
    clipboard: ClipboardContent | None
```

## Tensions

- **Interactive vs auto modes**: `lf/step.py:191-240` handles both, but interactive uses `os.execvp()` (replaces process) while flow execution uses `subprocess.run()` (continues after). The flow path needed special handling for interactive steps within flows.

- **Worktree lifecycle**: Waves use persistent worktrees (`wave.worktree`) but fork execution creates ephemeral worktrees. `lf/flow.py:265-340` manages cleanup, but the two models have different ownership semantics.

- **Token trimming vs context completeness**: `lf/context.py` drops large components (diff_files first) to stay under 200k tokens, but this can remove the very files the agent needs to see.

- **Daemon concurrency**: Manager limits concurrent steps (`max_concurrent_steps=2`) but doesn't have per-repo isolation. Two waves on the same repo could conflict.

## Observations

### Complexity

**`lf/flow.py` (540 lines)**: Dense orchestration logic. `run_flow()` handles Steps, Forks, and Choose in a single loop. Fork parallelism (`run_fork()`, `run_synthesize()`) adds ~150 lines. The topological batch execution for parallel steps adds more complexity.

**`lf/context.py` (718 lines)**: Context assembly with many code paths. `gather_prompt_components()` has 15+ parameters and calls 8 helper functions. Token trimming logic scattered across `trim_prompt_components()` and `analyze_components()`.

**`lfd/daemon/server.py`**: Asyncio server with method dispatch. Protocol is JSON-over-newline but not documented inline. Periodic checks (`_periodic_check()`) poll every 5s for wave triggers.

### Quality

**Strong typing throughout**: Pydantic models for config, dataclasses for internal state. Type hints on all public functions.

**Consistent patterns**: Verb-first functions (`find_prompt`, `load_config`), underscore prefix for private (`_run_step`), clean separation between parsing (`flows.py`) and execution (`flow.py`).

**Error messages vary in quality**:
- Good: `lf/config.py:89` - "Config file not found at {path}"
- Sparse: `lf/launcher.py:42` - just raises without context
- `lfd/daemon/server.py:156` logs errors but doesn't always surface them to users

**Documentation gaps**:
- Fork/synthesize algorithm not explained in code comments
- Tick-based execution (FlowRun.step_index) underdocumented
- Token trimming priority not explained (why diff_files first?)

**Test coverage**:
- `tests/test_context.py` (24KB) - thorough context assembly tests
- `tests/test_flows.py` - DAG building, parsing
- `tests/test_lfd.py` - Wave DB, stimulus logic
- Gap: Fork/synthesize execution lightly tested
- Gap: Interactive step handling within flows

### Potential

**Choose step**: Defined in `flows.py:53-62` but `flow.py` Choose handling (`choose_branch()`) is isolated. Could enable conditional workflows (if tests pass → ship, else → debug).

**Tick-based resumption**: `FlowRun.step_index` exists but isn't used for resumption after failures. Could enable "resume from step 3" functionality.

**Event streaming**: `lfd/daemon/protocol.py` defines Event type and server has subscribe method stub. Could power real-time UI without polling.

**Parallel step batches**: `topological_batches()` already groups independent steps. Currently runs in worktrees, but could run truly parallel with process isolation.

## Open questions

- Why does interactive mode in flows use `subprocess.run()` but standalone interactive uses `os.execvp()`? The comment at `flow.py:160` explains it, but is there a cleaner approach?

- Token trimming drops diff_files first. Is this always right? For a `debug` step, the diff is often the most important context.

- FlowRun.step_index tracks position but nothing uses it for resumption. Is this intended for future use or dead code?

- `lfd/execution/worker.py` exists but appears unused. Legacy or planned?

## Recommendations

### Clean up Choose handling

**Observation**: `flow.py:420-450` has `choose_branch()` that calls the runner directly rather than going through the standard step execution path. It writes to a file (`scratch/choices/{flow}.md`) and parses frontmatter.

**Cost**: Low—isolated change to one function.

**Benefit**: More consistent flow execution; Choose would benefit from the same logging/error handling as Steps.

**Verdict**: Worth doing. The special path is unnecessary complexity.

### Document token trimming strategy

**Observation**: `context.py:450-490` trims components without explaining priority. `_drop_candidates()` returns components in arbitrary order, then sorts by size.

**Cost**: Low—just comments.

**Benefit**: Maintainers understand why diff_files drops before clipboard. Users can anticipate behavior.

**Verdict**: Worth doing. Add a comment block explaining the priority and rationale.

### Remove or document worker.py

**Observation**: `lfd/execution/worker.py` exists but `runner.py` handles execution. Either worker.py is unused or its purpose is unclear.

**Cost**: Low—either delete or add docstring.

**Benefit**: Reduces confusion about execution architecture.

**Verdict**: Worth investigating. If unused, delete; if planned, document.

### Add fork/synthesize tests

**Observation**: `tests/test_flows.py` tests Flow parsing but not fork execution. `flow.py:run_fork()` and `run_synthesize()` have no dedicated tests.

**Cost**: Medium—need to mock worktree creation and parallel execution.

**Benefit**: Fork is a key feature (roadmap flows use it). Without tests, regressions go unnoticed.

**Verdict**: Worth doing before adding more fork features.
