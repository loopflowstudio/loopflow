# Research: Loopflow Codebase

## System understanding

Loopflow is an orchestration system for LLM coding agents (Claude Code, Codex, Gemini CLI). It assembles context and prompts, chains them into workflows, and manages autonomous "waves" of work. The core abstraction is **area × direction × flow × stimulus = wave**.

### Architecture

The codebase has four parallel implementations working together:

| Layer | Language | Purpose |
|-------|----------|---------|
| `lf` | Python | CLI for interactive prompt execution |
| `lfd` | Python | Daemon for autonomous wave orchestration |
| `lfops` | Python | Git/PR workflow automation |
| `lf-core` | Rust | Core engine for flow parsing and tick-based execution |

**Python CLIs** (src/loopflow/):
- `lf/` — Prompt launcher. Assembles context, runs steps interactively or in auto mode.
- `lfd/` — Daemon with Unix socket + gRPC + HTTP servers. Manages waves, triggers, worktrees.
- `lfops/` — Git operations: PR, land, rebase, worktree management.

**Rust Core** (rust/lf-core/):
- Flow parsing (YAML → data structures)
- Tick-based runtime for interactive step pausing
- Shells to `lf --step` for actual execution

**Swift UI** (swift/):
- Concerto: macOS app for wave management
- LoopflowCore: Shared models and services

### Data flow

```
User invokes lf/lfd
       │
       ▼
gather_prompt_components() — context.py
   ├── docs (scratch/, roadmap/, *.md)
   ├── diff_files (branch changes)
   ├── summaries (pre-generated)
   ├── direction (judgment/intent)
   └── step (the prompt itself)
       │
       ▼
format_prompt() → assembled prompt string
       │
       ▼
build_model_command() → claude/codex/gemini CLI command
       │
       ▼
collector subprocess → captures output, autocommits
```

For daemon-managed waves:
```
lfd daemon (server.py)
       │
       ├── Trigger evaluation (loop/watch/cron)
       │
       ▼
run_iteration() — runner.py
       │
       ├── create_worktree()
       ├── load_flow()
       │
       ▼
For each step/fork/choose:
       │
       ├── _build_loop_prompt()
       ├── _run_collector_step()
       └── autocommit + PR
```

### Key abstractions

| Concept | Definition | Location |
|---------|------------|----------|
| **Step** | A markdown prompt file with optional frontmatter | `lf/context.py:gather_step()` |
| **Flow** | Sequence of steps, forks, and choices | `lf/flows.py:Flow` |
| **Direction** | Perspective/judgment overlay | `lf/directions.py` |
| **Wave** | Autonomous work unit (area × direction × flow × stimulus) | `lfd/models.py:Wave` |
| **FlowRun** | Single execution of a flow | `lfd/models.py:FlowRun` |
| **StepRun** | Single execution of a step | `lfd/models.py:StepRun` |
| **PromptComponents** | Pre-assembly context pieces | `lf/context.py:PromptComponents` |

**Flow items hierarchy:**
- `Step` — Single prompt execution
- `Fork` — Parallel execution with synthesis
- `Choose` — LLM-driven branch selection
- `LoopUntilEmpty` — Repeat until backlog empty

## Tensions

**Python ↔ Rust boundary.** The Rust engine exists but is underutilized. `tick_flow()` shells to `lf --step` rather than using native Rust execution. This creates process overhead and complicates error propagation. The Rust code parses flows but the Python daemon (`runner.py`) has its own parallel flow execution logic.

**Interactive vs auto mode.** The system supports both but they have different code paths. `_run_interactive_step()` vs `_run_step()` in `flow.py` differ in how they handle TTY, subprocess management, and context assembly. The interactive mode can't be used within daemon-orchestrated flows.

**Context trimming opacity.** `trim_prompt_components()` drops components to fit token limits, but the priority order isn't documented. Looking at `_drop_candidates()`:456-490 in context.py, the order is: docs → summaries → diff → clipboard. This could surprise users who expect clipboard content (their error message) to survive trimming.

**Worktree lifecycle.** Worktrees are created per-iteration but also can be persistent per-wave (`wave.worktree`). The cleanup logic in `runner.py:_cleanup_worktree()` deletes both worktree and remote branch. Stacking commands (`lfd next`, `lfd rebase`) assume persistent worktrees and base_branch tracking.

## Observations

### Complexity

**`lfd/cli.py`** (53KB) — The daemon CLI is large with many commands. Wave creation, configuration, triggering, stacking, and monitoring are all here. Would benefit from subcommand grouping.

**`lfd/execution/runner.py`** (1099 lines) — Handles full iteration lifecycle, fork execution, PR creation, auto-merge, and tick-based flow execution. Two distinct execution modes (`run_iteration()` and `tick_flow()`) with overlapping responsibilities.

**`lf/flow.py`** (1180 lines) — Flow execution is complex: DAG building, parallel worktree tasks, fork/synthesize, choose branches, loop-until-empty. The `run_flow()` function is 210 lines with nested while loops.

**Token trimming** (`context.py:237-284`) — The greedy dropping algorithm prioritizes by token count then name, but the component priority isn't explicit. If a user pastes a 50k token error, it might push out their design doc.

### Quality

**Error messages** are inconsistent. `runner.py` uses `notify_event("wave.error", ...)` for daemon events but raw print statements for user-facing errors. Some errors include context (`f"Step file not found: {step_name}"`) while others are terse (`"Flow is required"`).

**Test coverage** is substantial (34 test files, ~55KB for `test_lfd.py` alone). Tests focus on behavior rather than mocking internals. However, fork/synthesize execution has no dedicated tests per `roadmap/code-quality.md`.

**Documentation** is well-organized. CLAUDE.md, STYLE.md, PROMPT_STYLE.md provide clear guidelines. Each command has help text, though some (`--direction`) lack context about what directions are.

**Rust code** follows style guide: `thiserror` for errors, `Debug` derives, `expect()` over `unwrap()`. The `StepRunner` trait enables test injection. Integration tests exist in `tests/` directory.

### Potential

**Rust core could own more.** The Python daemon currently handles flow execution, but `tick_flow_with_runner()` in Rust has the same capability. Moving execution to Rust would reduce process spawning overhead and enable better error handling.

**Wave stacking** is newly implemented (`lfd next`, `lfd rebase`) but not yet integrated with the loop/watch modes. A wave could automatically stack on itself when hitting PR limits.

**Summaries are underutilized.** The summary system exists (`gather_summaries()`, `load_summary()`) but requires explicit configuration. Could auto-generate summaries for large areas that exceed token budgets.

**Skills system is extensible.** External skill sources (superpowers, SkillRegistry) plug in via `discover_skill_sources()`. The pattern could extend to remote flow definitions or direction libraries.

## Open questions

- How should Rust and Python execution paths converge? Currently both exist independently.
- What happens when a fork thread creates more files than the synthesizer can compare?
- Is `choose_branch()` calling the LLM runner directly the right pattern, or should it go through the step execution path?
- How do `worker.py` and `runner.py` relate? The worker file exists but runner handles execution.
- What's the migration path for changing Wave/FlowRun schema in production?

## Recommendations

### Document token trimming priority

**Observation**: `trim_prompt_components()` drops docs before clipboard, but this isn't documented. Users might expect their pasted error to be protected.

**Cost**: Low — add a comment block explaining the order and rationale.

**Benefit**: Users understand why their context got trimmed. Developers can reason about changes.

**Verdict**: Worth it. Clear priority documentation prevents surprises.

### Consolidate execution paths

**Observation**: `runner.py:run_iteration()` and `flow.py:run_flow()` have overlapping logic for step execution, fork handling, and choose branches. The Rust `tick_flow()` adds a third path.

**Cost**: Medium — requires understanding all execution modes and carefully merging.

**Benefit**: Single execution path reduces bugs, simplifies testing, enables Rust optimization.

**Verdict**: Worth planning but needs careful design doc first.

### Add fork/synthesize tests

**Observation**: Fork execution in `flow.py:run_fork()` and synthesis have no dedicated tests. This is a key feature for the roadmap flows.

**Cost**: Medium — need to mock worktree creation and parallel execution.

**Benefit**: Confidence in fork behavior, regression prevention for synthesis prompts.

**Verdict**: Worth it. Fork is used in production flows.

### Remove or document worker.py

**Observation**: `lfd/execution/worker.py` exists but `runner.py` handles all execution. Unclear if worker.py is unused or has a specific purpose.

**Cost**: Low — investigate then delete or add docstring.

**Benefit**: Reduced cognitive load, cleaner codebase.

**Verdict**: Worth investigating immediately.
