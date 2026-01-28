# Research: Loopflow Codebase

## System understanding

Loopflow is an orchestration system for running autonomous coding agents (Claude Code, Codex, Gemini CLI) at scale. It solves one core problem: **prompt and context construction**—assembling the right context, running reusable prompts, and chaining them into workflows.

The system has three execution tiers:
1. **Interactive CLI (`lf`)** — Single prompts with context assembly
2. **Flow DAGs (`lf flow`)** — Chained steps with parallel/fork/choose
3. **Autonomous waves (`lfd`)** — Background daemon with scheduling

A fourth component, **Concerto**, provides a native macOS UI for conducting agents visually.

### Architecture

```
loopflow/
├── lf/                     # Interactive prompt engine (CLI)
│   ├── cli.py             # Entry point: `lf <step>` or `lf <flow>`
│   ├── step.py            # Single step execution (719 lines)
│   ├── flow.py            # Flow DAG execution (1,191 lines)
│   ├── flows.py           # Flow data structures (576 lines)
│   ├── context.py         # Context assembly (1,077 lines)
│   ├── launcher.py        # Agent CLI builders (684 lines)
│   ├── config.py          # Configuration loading
│   ├── directions.py      # Direction/perspective resolution
│   ├── skills.py          # External skill discovery (398 lines)
│   ├── worktrees.py       # Git worktree management (468 lines)
│   └── builtins/          # Bundled steps, flows, directions
│       ├── steps/         # 24 markdown step definitions
│       ├── flows/         # 12 YAML flow definitions
│       └── directions/    # Role and value perspectives
│
├── lfd/                    # Daemon for autonomous waves
│   ├── cli.py             # Entry point: `lfd create|loop|watch|...` (1,326 lines)
│   ├── models.py          # Wave, FlowRun, StepRun (347 lines)
│   ├── wave.py            # Wave CRUD operations (771 lines)
│   ├── db.py              # SQLite database management
│   ├── daemon/
│   │   ├── server.py      # Asyncio Unix socket server (582 lines)
│   │   ├── http_server.py # HTTP API for Concerto (552 lines)
│   │   ├── manager.py     # Concurrency slots, PR limits
│   │   └── protocol.py    # JSON-over-newline protocol
│   └── execution/
│       ├── runner.py      # Wave iteration executor (1,098 lines)
│       ├── collector.py   # Output capture, autocommit (387 lines)
│       └── worker.py      # Process management (462 lines)
│
├── lfops/                  # Git workflow operations (13 commands)
│   ├── land.py            # PR landing (749 lines)
│   ├── summarize.py       # Commit summarization (648 lines)
│   ├── wt.py              # Worktree operations (365 lines)
│   ├── pr.py              # PR creation (139 lines)
│   ├── commit.py          # Auto-commit (221 lines)
│   └── [8 more commands]
│
└── swift/                  # Native macOS apps (~13,600 lines)
    ├── Concerto/          # Main UI app (~7,500 lines)
    │   ├── Views/         # SwiftUI components
    │   ├── State/         # @EnvironmentObject state
    │   └── Services/      # Business logic
    ├── LoopflowCore/      # Shared framework
    │   ├── Models/        # Wave, Worktree, etc.
    │   └── Services/      # Daemon communication
    └── Symphonia/         # Secondary app (placeholder)
```

**Key data flow:**

```
User → step/flow command → context.py (gather) → launcher.py (build CLI)
                                                        ↓
                                            Agent subprocess (claude/codex/gemini)
                                                        ↓
                                            collector.py (capture, autocommit)
                                                        ↓
                                            daemon events → Concerto UI
```

### Key abstractions

**Wave (lfd/models.py):** The central orchestration unit for autonomous work.

```python
Wave:
  id, name, repo              # Identity
  flow, direction, area       # What to run and where
  stimulus: Stimulus          # When to run (once/loop/watch/cron)
  status: WaveStatus          # IDLE/RUNNING/WAITING/ERROR
  worktree, branch            # Persistent worktree state
  pr_limit, merge_mode        # PR creation settings
  consecutive_failures        # Circuit breaker counter
```

**Stimulus (lfd/models.py):** Determines when a wave runs.
- `once`: single run (one-shot)
- `loop`: continuously until stopped
- `watch`: when files in area change on main
- `cron`: on schedule (with cron expression)

**FlowItem (lf/flows.py:116):** Union type for flow execution.

```python
FlowItem = Step | Fork | Choose | LoopUntilEmpty

Step:     name, after, model, direction, interactive
Fork:     threads[], step, synthesize
Choose:   options{}, prompt
LoopUntilEmpty: steps[], wave, max_iterations
```

**PromptComponents (lf/context.py:112-127):** The assembled context before formatting.

```python
PromptComponents:
  run_mode: str | None       # "auto" or "interactive"
  docs: [(Path, str)]        # Reference documentation
  diff: str | None           # Unified diff
  diff_files: [(Path, str)]  # Full file contents
  step: (name, content)      # The step prompt
  direction: [Direction]     # Perspective/role
  clipboard: ClipboardContent
  loopflow_doc: str          # Bundled LOOPFLOW.md
  summaries: [(Path, str)]   # Pre-generated summaries
  wave: WaveContext          # Wave context for roadmap
```

**ContextConfig (lf/context.py:164-200):** Specifies what to include.

```python
ContextConfig:
  diff_mode: DiffMode        # FILES/DIFF/NONE
  files: FilesetConfig       # paths, exclude, token_limit
  area: str | None           # Area path for scoped context
  wave: str | None           # Wave name for roadmap
  lfdocs: bool = True        # Include LOOPFLOW.md
  clipboard: bool = False    # Include clipboard
  budget_area: 50000         # Token limits
  budget_docs: 30000
  budget_diff: 20000
```

### Data flow

**Step execution path (lf/step.py):**

1. `gather_step()` finds step file (repo → global → builtin)
2. `resolve_step_config()` applies CLI > frontmatter > config priority
3. `gather_prompt_components()` collects context per ContextConfig
4. `trim_prompt_components()` applies token budgets
5. `format_prompt()` assembles into tagged sections
6. `build_model_command()` creates agent CLI invocation
7. Interactive: `os.execvp()` replaces process
8. Auto: `subprocess.Popen()` with collector subprocess

**Flow execution path (lf/flow.py):**

1. `load_flow()` parses YAML or autopromotes step to single-step flow
2. `run_flow()` iterates over `FlowItem[]` with `while i < len(items)`
3. Consecutive `Step` items batched via `build_step_dag()` + `topological_batches()`
4. Single-step batches: `_run_step()` directly
5. Multi-step batches: `_run_worktree_tasks()` creates parallel worktrees
6. Fork items: `run_fork()` → parallel threads → `run_synthesize()`
7. Choose items: `choose_branch()` → AI picks branch → splice into flow
8. `_finalize_flow()` handles PR creation and notifications

**Daemon execution path (lfd/execution/runner.py):**

1. `worker.py` manages process lifecycle (retries, circuit breaker)
2. `run_iteration()` creates worktree from `wave.main_branch`
3. FlowRun tracked in database
4. Iterates flow items similar to flow.py but with:
   - 30-min timeout (`StepTimeoutError`)
   - PR creation to wave's main branch
   - Event notifications via daemon socket
5. On success: create PR, auto-merge if configured
6. Cleanup: remove worktree, push deleted branch

**Concerto UI data flow:**

1. `LFDEventService` connects to daemon Unix socket
2. Subscribes to event patterns (wave.*, session.*, worktree.*)
3. `WaveService` / `WorktreeService` query HTTP API
4. SwiftUI views observe `@Published` state changes
5. Actions dispatch to daemon via socket protocol

### Dependencies between modules

**From lf to lfd:**
- `lfd/models.py`: StepRun, StepRunStatus (used in step.py, flow.py)
- `lfd/step_run.py`: log_step_run_start/end

**From lfd to lf:**
- `lf/context.py`: gather_prompt_components, format_prompt
- `lf/flows.py`: Flow, Step, Fork, load_flow
- `lf/launcher.py`: build_model_command, get_runner
- `lf/worktrees.py`: create, remove
- `lf/directions.py`: resolve_directions
- `lf/config.py`: load_config, parse_model

**External tools:**
- `wt` (worktrunk) for worktree management
- `gh` for PR operations
- `claude`, `codex`, `gemini` CLIs for agent execution

---

## Tensions

**Dual execution paths:** `lf/flow.py:run_flow()` (1,191 lines) and `lfd/execution/runner.py:run_iteration()` (1,098 lines) implement similar flow execution logic with subtle differences:
- CLI version: no timeout, optional PR creation, direct subprocess
- Daemon version: 30-min timeout, mandatory PR to wave branch, event notifications
- Both handle Step, Fork, Choose items but with different error handling

The duplication is significant (~60% overlap in step/fork/choose handling) and creates maintenance burden—changes often need to be made in both files.

**Interactive vs. auto mode:** Steps declare `interactive: true` in frontmatter. Detection happens at `_is_step_interactive()` (flow.py:90-100) with priority:
1. Flow step override (explicit in flow YAML)
2. Step frontmatter
3. Default (False)

Interactive steps bypass the collector subprocess and use `subprocess.run()` to preserve TTY. The transition between modes within a flow works but the boundary is not obvious from reading the code.

**Choose bypasses standard execution:** `choose_branch()` (flow.py:937-969) calls `runner.launch()` directly rather than going through `_run_step()`. This bypasses:
- Step run logging
- Token warnings
- Collector subprocess
- Event notifications

Debugging issues with Choose is harder because it doesn't leave the same audit trail.

**Worktree naming conventions diverge:** Four patterns coexist without documentation:
- Fork worktrees: `fork-{flow_name}-{index}` (flow.py:576)
- Parallel step worktrees: `_parallel-{step_name}-{uuid8}` (flow.py:1055)
- General worktree tasks: `{wt_prefix}-{label_short}-{uuid8}` (flow.py:428)
- Daemon parallel worktrees: `parallel-{branch}-{step_name}-{index}` (runner.py:676)

Cleanup logic must understand all four patterns, making it fragile.

**Token budgeting is implicit:** `ContextConfig` has budget_area (50k), budget_docs (30k), budget_diff (20k) but the trimming strategy in `trim_prompt_components()` (context.py:237-284) drops components in a specific order without clear rationale documented:
1. Drop diff_files if it alone exceeds limit
2. Greedy by size from: docs, summaries, diff, clipboard
3. Last resort: drop diff_files entirely

The ordering affects what context the agent sees, but there's no explanation of why this order was chosen.

**Worker vs Runner responsibility:** `worker.py` (462 lines) and `runner.py` (1,098 lines) have overlapping concerns:
- Worker: Process lifecycle, retries, circuit breaker, PR limits
- Runner: Iteration logic, step execution, fork/synthesize

The division isn't immediately clear from the code structure.

---

## Observations

### Complexity hotspots

**flow.py (1,191 lines):** The core flow execution engine handles four flow item types with index-based iteration:
- `run_flow()`: 205 lines with `while i < len(items)` and complex control flow
- `run_fork()`: 180 lines managing parallel worktrees and synthesis
- `_run_worktree_tasks()`: 120 lines for parallel step execution
- `choose_branch()`: 35 lines that bypass standard execution

The main loop's index-based iteration with manual advancement (`i += len(batch)`) is error-prone and hard to extend.

**context.py (1,077 lines):** Context assembly handles 10+ different context sources:
- `gather_prompt_components()`: 140 lines with interdependencies
- `trim_prompt_components()`: 50 lines with greedy dropping
- `format_prompt()`: 120 lines assembling final output
- Multiple `gather_*` helper functions

The function `gather_prompt_components()` has complex conditional logic based on wave, area, and other config options that interact in non-obvious ways.

**runner.py (1,098 lines):** The daemon execution engine duplicates much of flow.py:
- `run_iteration()`: 314 lines paralleling run_flow()
- `_run_fork_synthesize()`: mirrors flow.py's run_fork()
- `_build_loop_prompt()`: duplicates context assembly patterns
- `tick_flow()`: state machine for interactive step support

### Quality variations

**Step run tracking:** Consistent across interactive and auto paths via `log_step_run_start/end()`. Exception: `choose_branch()` has no StepRun tracking at all.

**Error handling patterns:**
- flow.py: Returns first non-zero exit code, cleanup on failure via finally blocks
- runner.py: Additional retry/backoff via circuit breaker (`consecutive_failures`)
- daemon/server.py: Graceful error responses via protocol, but errors can be cryptic

**Documentation quality:**
- CLAUDE.md/STYLE.md: Excellent, comprehensive coding guidelines
- PROMPT_STYLE.md: Clear prompt writing standards
- VISUAL_DESIGN.md: Thorough design system documentation
- Module-level docstrings: Inconsistent—some files have none

**Test coverage (tests/):** 34 test files, 9,683 lines covering key areas:
- Flow parsing and DAG construction (test_flows.py) - 139 lines
- Context gathering (test_context.py) - 750 lines
- Worktree operations (test_worktrees.py) - 350 lines
- Launcher backends (test_launcher.py) - 582 lines
- lfd CLI and daemon (test_lfd.py) - 1,756 lines

Missing dedicated tests for:
- `run_fork()` and `run_synthesize()` execution paths
- `choose_branch()` execution
- Parallel worktree tasks (`_run_worktree_tasks`)
- Error paths and cleanup in flow execution
- Token trimming edge cases

### Potential

**Agent backend abstraction:** `launcher.py` has a clean `Runner` ABC with `ClaudeRunner`, `CodexRunner`, `GeminiRunner`. Normalized event streaming works across backends via `normalize_*_event()` functions. Adding new backends would be straightforward.

**Skill system extensibility:** `skills.py` supports external sources (superpowers, SkillRegistry). Skills use SKILL.md format compatible with Claude Code's skills standard. The discovery mechanism (`discover_skill_sources()`) is well-designed and extensible.

**Token budgeting infrastructure:** Budget fields exist on ContextConfig with reasonable defaults. The trimming infrastructure is in place—just needs better documentation and potentially smarter strategies (e.g., prioritize recent changes over old docs).

**Concerto UI architecture:** The Swift app follows solid design principles from DESIGN.md:
- Immediate connection (streaming output)
- Progressive disclosure (expandable details)
- Keyboard-first (Cmd+K command palette)
- Speed as feature (optimistic updates)

The separation between LoopflowCore services and Concerto views is clean.

**Protocol-first design:** Proto definitions exist in `proto/` for a potential Rust rewrite. The JSON-over-newline protocol is simple and debuggable. Event categories are well-structured (wave.*, session.*, worktree.*).

---

## Open questions

- **Choose output path collision:** Multiple flows using `Choose` write to `scratch/choices/{flow_name}.md`. If flow names collide across repos, choices could overwrite. Is this intentional for debugging, or a bug waiting to happen?

- **Interactive steps in parallel batches:** What happens if a multi-step DAG batch includes an interactive step? Currently `_run_worktree_tasks()` always uses auto mode (flow.py:448). Is this documented anywhere?

- **Timeout handling asymmetry:** Daemon has 30-min step timeout with process tree cleanup. CLI has no timeout—stuck agents run forever. Should CLI flows also timeout?

- **Fork thread step inheritance:** The logic at `step_name = thread.step or fork.step` (flow.py:591-592) is implicit. Should this be documented or made explicit in the ForkThread dataclass?

- **Cron grace period:** `check_cron_stimulus()` has a 24-hour grace period for missed triggers. Is this configurable? What's the rationale for 24 hours specifically?

---

## Recommendations

### Document token trimming strategy

**Observation:** `trim_prompt_components()` (context.py:237-284) drops components in a specific order but the rationale isn't documented. Drop priority: diff_files first (if over limit alone), then greedy by size from docs/summaries/diff/clipboard.

**Cost:** Low (comments only).

**Benefit:** Maintainers understand why certain context is dropped first. Users can predict behavior when hitting limits.

**Verdict:** Worth adding for clarity.

### Route choose_branch() through standard execution

**Observation:** `choose_branch()` calls `runner.launch()` directly, bypassing step run logging, token warnings, and the collector subprocess.

**Cost:** Low. Either create an inline prompt execution path that logs, or create a `_choose.md` synthetic step.

**Benefit:** Consistent execution semantics. Step run tracking for debugging. Token warnings shown.

**Verdict:** Worth it if Choose is used in production flows.

### Add execution tests for fork/synthesize

**Observation:** `run_fork()` and `run_synthesize()` have no dedicated unit tests. The only coverage is through integration flows.

**Cost:** Medium. Need to mock worktree creation and parallel execution.

**Benefit:** Confidence in complex code paths. Regression protection for fork, which is a key feature for roadmap flows.

**Verdict:** Worth it—fork is central to the roadmap-reduce/polish/expand flows.

### Document worktree naming conventions

**Observation:** Four different naming patterns across flow.py and runner.py with no documentation.

**Cost:** Low (comments in each location, plus a reference in STYLE.md).

**Benefit:** Maintainers understand which worktrees to expect. Cleanup logic becomes more obvious.

**Verdict:** Worth it for clarity.

### Add timeout support to CLI flow execution

**Observation:** Daemon has 30-min step timeout with `StepTimeoutError` and process tree cleanup. CLI has no timeout—stuck agents run forever.

**Cost:** Medium. Port `StepTimeoutError` handling from runner.py to flow.py's `_run_step()`.

**Benefit:** Prevents runaway agents in CLI use. Consistent behavior across execution paths.

**Verdict:** Worth adding for reliability.

### Extract shared step preparation logic

**Observation:** `_run_step()` (flow.py:194-286) and `_run_interactive_step()` (flow.py:103-191) duplicate context gathering, token warnings, and step run logging (~60% overlap).

**Cost:** Low. Extract to `_prepare_step() -> (prompt, step_run, components)`.

**Benefit:** Reduced code duplication. Single place to modify step preparation.

**Verdict:** Worth it as part of any flow.py changes.

### Document worker.py vs runner.py division

**Observation:** `worker.py` (462 lines) handles process lifecycle, `runner.py` (1,098 lines) handles iteration logic. The division isn't documented.

**Cost:** Low (docstrings only).

**Benefit:** Clear mental model for where to add new functionality. Worker = "what to run and when", Runner = "how to run it".

**Verdict:** Worth it for clarity.

### Consider consolidating flow.py and runner.py execution

**Observation:** `runner.py` duplicates significant flow execution logic from `flow.py`, adding daemon-specific features. The duplication creates ongoing maintenance burden.

**Cost:** High. Careful refactoring to unify while preserving:
- Daemon: timeout, PR creation, event notifications, tick-based execution
- CLI: no timeout, optional PR, direct subprocess

**Benefit:** Single source of truth for flow execution. Easier to add features to both paths simultaneously.

**Verdict:** Worth exploring as a larger refactoring effort. Not urgent, but track as technical debt. A possible approach: extract a shared `FlowExecutor` class that both paths use, with hooks for daemon-specific behavior.
