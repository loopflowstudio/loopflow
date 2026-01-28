# Research: Loopflow Codebase

## System understanding

Loopflow is an orchestration system for running autonomous coding agents (Claude Code, Codex, Gemini CLI) at scale. It solves one core problem: **prompt and context construction**—assembling the right context, running reusable prompts, and chaining them into workflows. The system has three execution tiers: interactive CLI (`lf`), flow DAGs (`lf flow`), and autonomous waves (`lfd`).

### Architecture

```
loopflow/
├── lf/                     # Interactive prompt engine (CLI)
│   ├── cli.py             # Entry point: `lf <step>` or `lf <flow>`
│   ├── step.py            # Single step execution
│   ├── flow.py            # Flow DAG execution (1178 lines)
│   ├── flows.py           # Flow data structures (577 lines)
│   ├── context.py         # Context assembly (1078 lines)
│   ├── launcher.py        # Agent CLI builders (685 lines)
│   ├── config.py          # Configuration loading
│   ├── directions.py      # Direction/perspective resolution
│   ├── files.py           # File gathering utilities
│   ├── tokens.py          # Token counting and budgeting
│   ├── worktrees.py       # Git worktree management (via worktrunk)
│   └── builtins/          # Bundled steps, flows, directions
│
├── lfd/                    # Daemon for autonomous waves
│   ├── cli.py             # Entry point: `lfd create|loop|watch|...`
│   ├── models.py          # Wave, FlowRun, StepRun data structures
│   ├── wave.py            # Wave CRUD operations
│   ├── db.py              # SQLite database management
│   ├── daemon/
│   │   ├── server.py      # Asyncio Unix socket server
│   │   ├── http_server.py # HTTP API for Concerto
│   │   ├── manager.py     # Concurrency slots, PR limits
│   │   └── protocol.py    # JSON-over-newline protocol
│   └── execution/
│       ├── runner.py      # Wave iteration executor (1099 lines)
│       ├── collector.py   # Output capture, autocommit
│       └── worker.py      # Process management
│
└── lfops/                  # Git workflow operations
    ├── commands.py        # Command registration
    ├── pr.py, land.py     # PR workflow
    ├── commit.py          # Auto-commit
    └── wt.py              # Worktree operations
```

**Key data flow:**

```
User prompt/step → context.py (gather + assemble) → launcher.py (build CLI)
                                                           ↓
                                               Agent subprocess (claude/codex/gemini)
                                                           ↓
                                               collector.py (capture, autocommit)
```

### Key abstractions

**Wave (lfd/models.py:68-174):** The central orchestration unit.

```python
Wave:
  id, name, repo              # Identity
  flow, direction, area       # What to run and where
  stimulus: Stimulus          # When to run (once/loop/watch/cron)
  status: WaveStatus          # idle/running/waiting/error
  worktree, branch            # Persistent worktree state
  pr_limit, merge_mode        # PR creation settings
```

**Flow/FlowItem (lf/flows.py:116-127):** DAG-based step orchestration.

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
  diff_mode: DiffMode        # files/diff/none
  files: FilesetConfig       # paths, exclude, token_limit
  area: str | None           # Area path for scoped context
  wave: str | None           # Wave name for roadmap
  budget_area: 50000         # Token limits
  budget_docs: 30000
  budget_diff: 20000
```

### Data flow

**Step execution path (lf/step.py + lf/flow.py):**

1. `gather_step()` finds step file (repo → global → builtin)
2. `gather_prompt_components()` collects context per ContextConfig
3. Token budgets applied via `_limit_to_budget()`
4. `format_prompt()` assembles into tagged sections (`<lf:step>`, `<lf:docs>`, etc.)
5. `build_model_command()` creates agent CLI invocation
6. Subprocess runs with output streaming to collector

**Flow execution path (lf/flow.py:972-1177):**

1. `load_flow()` parses YAML or autopromotes step to single-step flow
2. `run_flow()` iterates over `FlowItem[]`
3. Consecutive `Step` items batched via `build_step_dag()` + `topological_batches()`
4. Single-step batches: `_run_step()` directly
5. Multi-step batches: `_run_worktree_tasks()` creates parallel worktrees, merges results
6. Fork items: `run_fork()` → parallel threads → `run_synthesize()`
7. Choose items: `choose_branch()` → splice selected branch into flow

**Daemon execution path (lfd/execution/runner.py:504-818):**

1. `run_iteration()` creates worktree from `wave.main_branch`
2. FlowRun tracked in database
3. Iterates flow items similar to `lf/flow.py` but with:
   - Timeout handling (`StepTimeoutError`, 30 min default)
   - PR creation to wave's main branch
   - Event notifications via daemon socket
4. Results merge, worktree cleanup

### Dependencies

**From lf to lfd:**
- `lfd/models.py`: StepRun, StepRunStatus (used in step.py, flow.py)
- `lfd/step_run.py`: log_step_run_start/end

**From lfd to lf:**
- `lf/context.py`: gather_prompt_components, format_prompt
- `lf/flows.py`: Flow, Step, Fork, load_flow
- `lf/launcher.py`: build_model_command, get_runner
- `lf/worktrees.py`: create, remove

**External tools:**
- `wt` (worktrunk) for worktree management
- `gh` for PR operations
- `claude`, `codex`, `gemini` CLIs for agent execution

## Tensions

**Dual execution paths:** `lf/flow.py:run_flow()` and `lfd/execution/runner.py:run_iteration()` implement similar flow execution logic with subtle differences:
- CLI version: no timeout, optional PR creation, direct subprocess
- Daemon version: 30-min timeout, mandatory PR to wave branch, event notifications
- Both handle Step, Fork, Choose items but with different error handling

**Interactive vs. auto mode:** Steps declare `interactive: true` in frontmatter. Detection happens at `_is_step_interactive()` (flow.py:90-100) with priority: flow step override > frontmatter > default. Interactive steps bypass the collector subprocess.

**Choose bypasses standard execution:** `choose_branch()` (flow.py:937-969) calls `runner.launch()` directly rather than going through `_run_step()`. This bypasses step run logging, token warnings, and collector subprocess.

**Worktree naming conventions diverge:** Four patterns coexist without documentation:
- Fork worktrees: `fork-{flow_name}-{index}` (flow.py:576)
- Parallel step worktrees: `_parallel-{step_name}-{uuid8}` (flow.py:1055)
- General worktree tasks: `{wt_prefix}-{label_short}-{uuid8}` (flow.py:428)
- Daemon parallel worktrees: `parallel-{branch}-{step_name}-{index}` (runner.py:676)

**Token budgeting is implicit:** `ContextConfig` has budget_area (50k), budget_docs (30k), budget_diff (20k) but the trimming strategy (`trim_prompt_components`, context.py:237-284) drops components in a specific order without external documentation. Drop priority: diff_files first, then greedy by size from docs/summaries/diff/clipboard.

## Observations

### Complexity

**run_flow() (flow.py:972-1177):** 205 lines handling four flow item types with index-based iteration and manual advancement. The main loop uses `while i < len(items)` with complex control flow for each item type.

**Context assembly (context.py:808-948):** `gather_prompt_components()` is 140 lines handling 10+ different context sources with interdependencies (wave affects exclude patterns, area affects doc gathering, etc.).

**runner.py:run_iteration (504-818):** 314 lines duplicating much of flow.py's logic with daemon-specific additions. Functions like `_run_fork_synthesize()`, `_build_loop_prompt()` mirror flow.py implementations.

### Quality

**Step run tracking:** Consistent across interactive and auto paths via `log_step_run_start/end()`. Exception: `choose_branch()` has no StepRun tracking.

**Error handling patterns:**
- flow.py: Returns first non-zero exit code, cleanup on failure
- runner.py: Additional retry/backoff via circuit breaker (`consecutive_failures`)
- daemon/server.py: Graceful error responses via protocol

**Test coverage (tests/):** 34 test files covering:
- Flow parsing and DAG construction (test_flows.py)
- Context gathering (test_context.py)
- Worktree operations (test_worktrees.py)
- Configuration (test_config.py)
- Git operations (test_git.py)
- PR polling (test_pr_poller.py)
- lfd CLI (test_lfd_cli.py)

Missing dedicated tests for:
- `run_fork()`, `run_synthesize()` execution
- `choose_branch()` execution
- Parallel worktree tasks (`_run_worktree_tasks`)
- Error paths and cleanup in flow execution

### Potential

**Unified step preparation:** `_run_step()` and `_run_interactive_step()` share ~60% of code (context gathering, step run logging, token warnings). A shared `_prepare_step() -> (prompt, step_run, components)` could reduce duplication.

**Agent backend abstraction:** `launcher.py` has clean `Runner` ABC with `ClaudeRunner`, `CodexRunner`, `GeminiRunner`. Normalized event streaming works across backends.

**Skill system extensibility:** `skills.py` supports external sources (superpowers, SkillRegistry). Skills use SKILL.md format compatible with Claude Code's skills standard.

**Token budgeting is explicit:** Budget fields exist on ContextConfig. Infrastructure for smarter trimming is in place.

## Open questions

- **Choose output path collision:** Multiple flows using `Choose` write to `scratch/choices/{flow_name}.md`. If flow names collide, choices overwrite. Intentional for debugging, or a bug?

- **Interactive steps in parallel batches:** What happens if a multi-step batch includes an interactive step? Currently `_run_worktree_tasks()` always uses auto mode (flow.py:448).

- **Timeout handling asymmetry:** Daemon has 30-min step timeout. CLI has none. Should CLI flows also timeout?

- **Fork thread step inheritance:** Implicit logic at `step_name = thread.step or fork.step` (flow.py:591-592). Should this be documented or made explicit in the data structure?

## Recommendations

### Extract shared step preparation logic

**Observation:** `_run_step()` (flow.py:194-286) and `_run_interactive_step()` (flow.py:103-191) duplicate context gathering, token warnings, and step run logging.

**Cost:** Low. Extract to `_prepare_step() -> (prompt, step_run, components)`.

**Benefit:** Reduced code duplication. Single place to modify step preparation.

**Verdict:** Worth it as part of any flow.py changes.

### Route choose_branch() through standard execution

**Observation:** `choose_branch()` calls `runner.launch()` directly, bypassing step run logging and token warnings.

**Cost:** Low. Create inline prompt execution path or `choose.md` step.

**Benefit:** Consistent execution semantics. Step run tracking for debugging.

**Verdict:** Worth it if Choose is used in production flows.

### Add execution tests for fork/synthesize

**Observation:** `run_fork()` and `run_synthesize()` have no dedicated tests. Tested only through integration flows.

**Cost:** Medium. Need to mock worktree creation and parallel execution.

**Benefit:** Confidence in complex code paths. Regression protection.

**Verdict:** Worth it—fork is a key feature for roadmap flows.

### Document worktree naming conventions

**Observation:** Four different naming patterns across flow.py and runner.py with no documentation.

**Cost:** Low (comments only).

**Benefit:** Maintainers understand which worktrees to expect. Cleanup logic clearer.

**Verdict:** Worth it for clarity.

### Add timeout support to CLI flow execution

**Observation:** Daemon has 30-min step timeout with process tree cleanup. CLI has no timeout—stuck agents run forever.

**Cost:** Medium. Port `StepTimeoutError` handling from runner.py to flow.py.

**Benefit:** Prevents runaway agents in CLI use.

**Verdict:** Worth adding for reliability.

### Consider consolidating flow.py and runner.py execution

**Observation:** `runner.py` (1099 lines) duplicates significant flow execution logic from `flow.py` (1178 lines), adding daemon-specific features. The duplication creates maintenance burden.

**Cost:** High. Careful refactoring to unify while preserving daemon-specific behavior.

**Benefit:** Single source of truth for flow execution. Easier to add features to both paths.

**Verdict:** Worth exploring as larger refactoring effort, not urgent. Track as technical debt.

### Document token trimming strategy

**Observation:** `trim_prompt_components()` drops components in a specific order but the rationale isn't documented. Priority: diff_files → (docs, summaries, diff, clipboard sorted by size).

**Cost:** Low (comments only).

**Benefit:** Maintainers understand why certain context is dropped first.

**Verdict:** Worth adding for clarity.
