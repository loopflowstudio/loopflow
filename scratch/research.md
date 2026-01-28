# Research: flow.py — Flow Execution Engine

## System understanding

`flow.py` is the execution engine for chained agent workflows. It orchestrates step execution, parallel worktree management, and fork/synthesize patterns. The module bridges three subsystems: flow definitions (`flows.py`), context assembly (`context.py`), and agent launching (`launcher.py`).

### Architecture

```
flow.py (1178 lines)
├── Step execution (_run_step, _run_interactive_step)
├── Parallel execution (_run_worktree_tasks)
├── Fork execution (run_fork)
├── Synthesize execution (run_synthesize)
├── Choose branch (choose_branch)
├── Loop until empty (LoopUntilEmpty handler)
└── Main orchestrator (run_flow)

flows.py (577 lines)
├── Data structures (Step, Fork, Choose, LoopUntilEmpty)
├── DAG construction (build_step_dag, StepDAG)
├── Flow loading/saving (load_flow, save_flow)
├── Validation (validate_flows)
└── Listing (list_flows, list_steps, list_directions)
```

**Key data flow:**

```
Flow (YAML) → FlowItem[] → run_flow() → _run_step() → collector subprocess
                                ↓
                         worktree creation → parallel execution → merge
```

### Key abstractions

**FlowItem union type (flow.py:49, flows.py:116):** `Step | Fork | Choose | LoopUntilEmpty`

Each variant has distinct execution semantics:
- `Step`: Single agent invocation with optional model/direction overrides
- `Fork`: Parallel execution across temporary worktrees with synthesis
- `Choose`: LLM-driven branch selection based on repo state
- `LoopUntilEmpty`: Iterative execution until wave backlog empties

**StepDAG (flows.py:156-160):** Dependency graph for parallel step batching. The `after:` field creates edges; steps without explicit dependencies follow the previous step.

**_StepParams (flow.py:52-61):** Normalized parameters for step execution after applying overrides from step config, flow defaults, and CLI flags.

### Data flow

**Step execution path:**

1. `run_flow()` iterates over `FlowItem[]` (line 1002)
2. Consecutive `Step` items collected into phases (lines 1006-1009)
3. Phase becomes `StepDAG` via `build_step_dag()` (line 1011)
4. `topological_batches()` groups steps by dependency level (line 1012)
5. Single-step batches run via `_run_step()` (lines 1015-1044)
6. Multi-step batches spawn parallel worktrees via `_run_worktree_tasks()` (lines 1047-1081)
7. Results merge back to parent worktree (lines 1076-1078)

**Fork execution path:**

1. `run_fork()` creates worktrees from `base_commit` (lines 558-647)
2. Each `ForkThread` runs in parallel via `ThreadPoolExecutor` (lines 630-645)
3. Results collected as `ForkResult` with diff, status, scratch notes
4. `run_synthesize()` prompts agent to unify results (lines 650-677)
5. `cleanup_fork_worktrees()` removes temporary worktrees (lines 680-684)

**Choose execution path:**

1. `choose_branch()` builds prompt with available options (lines 937-969)
2. Agent writes choice to `scratch/choices/{flow}.md` with YAML frontmatter
3. Selected branch's steps spliced into flow items (line 1134)
4. Execution continues with expanded items

### Dependencies

**From loopflow.lf:**
- `context.py`: `gather_prompt_components()`, `format_prompt()`, `trim_prompt_components()`
- `flows.py`: `Flow`, `Step`, `Fork`, `Choose`, `build_step_dag()`
- `launcher.py`: `build_model_command()`, `get_runner()`
- `worktrees.py`: `create()`, `remove()`
- `git.py`: `find_main_repo()`, `open_pr()`

**From loopflow.lfd:**
- `models.py`: `StepRun`, `StepRunStatus`
- `step_run.py`: `log_step_run_start()`, `log_step_run_end()`
- `execution/collector.py`: Subprocess for output capture, autocommit
- `execution/runner.py`: Duplicates much of flow.py for daemon context
- `execution/worker.py`: Coordinates with daemon manager for concurrency

## Tensions

**Interactive vs. auto mode:** Steps can override the default run mode via frontmatter (`interactive: true`). Detection at `_is_step_interactive()` (lines 90-100) with priority: flow step override > frontmatter > default. Interactive steps use direct subprocess; auto steps use the collector wrapper.

**Parallel worktree lifecycle:** `_run_worktree_tasks()` creates temporary worktrees for parallel batches, merges results, then cleans up (lines 1076-1081). Cleanup can fail mid-way if merges conflict.

**Choose bypasses standard execution:** `choose_branch()` (lines 937-969) calls `runner.launch()` directly rather than going through `_run_step()`. This bypasses:
- Step run logging (`log_step_run_start/end`)
- Token limit warnings
- Collector subprocess (autocommit, output streaming)

**Fork thread vs. fork-level step:** Fork threads can specify their own `step` or inherit from `fork.step` (line 591-592). The inheritance logic is implicit: `step_name = thread.step or fork.step`.

**Dual execution paths:** `flow.py:run_flow()` handles CLI/direct execution while `lfd/execution/runner.py:run_iteration()` handles daemon execution. Both implement similar flow item handling with subtle differences—the daemon version has timeout handling, PR creation to wave's main branch, and event notifications.

## Observations

### Complexity

**run_flow() (lines 972-1177):** 205 lines handling four flow item types. The main loop uses index-based iteration with manual advancement, making control flow hard to follow. Each item type has distinct handling:
- Steps: collected into phases, batched, may parallelize (lines 1005-1083)
- Fork: spawn worktrees, run threads, synthesize (lines 1085-1122)
- Choose: prompt agent, splice result into items list (lines 1124-1136)
- LoopUntilEmpty: recursive run_flow calls (lines 1138-1172)

**Token trimming integration:** Steps call `trim_prompt_components()` (lines 131, 219, 454) to fit context within `MAX_SAFE_TOKENS`. Warning messages print but execution continues. The trimming priority order is documented in code but not externally.

**Worktree naming:** Multiple conventions coexist:
- Fork worktrees: `fork-{flow_name}-{index}` (flow.py:576)
- Parallel step worktrees: `_parallel-{step_name}-{uuid8}` (flow.py:1055)
- General worktree tasks: `{wt_prefix}-{label_short}-{uuid8}` (flow.py:428)
- Daemon parallel worktrees: `parallel-{branch}-{step_name}-{index}` (runner.py:676)

### Quality

**Step run tracking:** All step executions log start/end via `log_step_run_start/end()`. Interactive and auto paths both track status. Choose is the exception—no StepRun tracking.

**Error handling:** Failures propagate up immediately. `run_flow()` returns first non-zero exit code (line 984 docstring). Fork failures trigger cleanup before returning (lines 1101-1103, 1116-1118). The daemon's `runner.py` has additional retry/backoff and circuit breaker logic.

**Test coverage:** `test_flows.py` has 8 tests covering:
- Flow parsing from dict
- DAG construction with default/explicit after
- Topological batching for parallel steps
- Flow loading from YAML
- Fork parsing and thread inheritance
- Save/load roundtrip

No dedicated tests for:
- `run_fork()` execution
- `run_synthesize()` execution
- `choose_branch()` execution
- Parallel worktree tasks (`_run_worktree_tasks`)
- Error paths and cleanup

### Potential

**Unified execution path:** Choose could route through standard execution machinery. A step like `choose.md` with special frontmatter could handle the prompt/parse cycle while gaining logging and autocommit.

**Step execution abstraction:** `_run_step()` and `_run_interactive_step()` share ~60% of their code (context gathering, step run logging, token warnings). A shared `_prepare_step()` could reduce duplication.

**Fork parallelism is configurable:** `MAX_FORK_THREADS = 5` in `flows.py`. Fork already uses `ThreadPoolExecutor(max_workers=len(fork.threads))`, so it naturally scales.

**Consolidate flow.py and runner.py:** The daemon's `runner.py` (1099 lines) duplicates significant flow execution logic. Functions like `_run_fork_synthesize()`, `_build_loop_prompt()`, and the main iteration loop mirror `flow.py`. A unified execution layer could serve both CLI and daemon contexts.

## Open questions

- **Choose output path collision:** Multiple flows using `Choose` write to `scratch/choices/{flow_name}.md`. If flow names collide, choices overwrite. Is this intentional for debugging, or a bug?

- **Interactive steps in parallel batches:** What happens if a multi-step batch includes an interactive step? Currently `_run_worktree_tasks()` always uses auto mode (line 448: `run_mode="auto"`).

- **LoopUntilEmpty wave determination:** `_get_wave_name()` falls back to worktree directory name or branch. In a fork worktree, this may not match the intended wave.

- **Timeout handling asymmetry:** Daemon execution has `DEFAULT_STEP_TIMEOUT = 30 * 60` with `StepTimeoutError`. CLI execution via `flow.py` has no timeout. Should CLI flows also timeout?

## Recommendations

### Route choose_branch() through standard execution

**Observation:** `choose_branch()` (lines 937-969) calls `runner.launch()` directly, bypassing step run logging, token warnings, and collector subprocess.

**Cost:** Low. The function is isolated. Refactor to use `_run_inline_prompt()` or create a `choose.md` step that handles parsing.

**Benefit:** Consistent execution semantics. Step run tracking for debugging. Autocommit if desired.

**Verdict:** Worth it if Choose is used in production flows.

### Add execution tests for fork/synthesize

**Observation:** `run_fork()` and `run_synthesize()` have no dedicated tests. They're tested only through integration flows.

**Cost:** Medium. Need to mock worktree creation and parallel execution.

**Benefit:** Confidence in the most complex code path. Regression protection.

**Verdict:** Worth it—fork is a key feature for roadmap flows.

### Extract shared step preparation logic

**Observation:** `_run_step()` (lines 194-286) and `_run_interactive_step()` (lines 103-191) duplicate context gathering, token warnings, and step run logging.

**Cost:** Low. Extract to `_prepare_step() -> (prompt, step_run, components)`.

**Benefit:** Reduced code duplication. Single place to modify step preparation.

**Verdict:** Worth it as part of any future flow.py changes.

### Document worktree naming conventions

**Observation:** Four different naming patterns for temporary worktrees across `flow.py` and `runner.py`. No documentation explains the prefixes or cleanup expectations.

**Cost:** Low (comments only).

**Benefit:** Maintainers understand which worktrees to expect. Cleanup logic clearer.

**Verdict:** Worth it—clarity investment.

### Consider consolidating flow.py and runner.py

**Observation:** `lfd/execution/runner.py` (1099 lines) duplicates much of `flow.py`'s flow execution logic, adding daemon-specific features (timeouts, PR creation, events). The duplication creates maintenance burden and risks divergence.

**Cost:** High. Would require careful refactoring to unify while preserving daemon-specific behavior.

**Benefit:** Single source of truth for flow execution. Easier to add features (like timeouts) to both paths.

**Verdict:** Worth exploring as a larger refactoring effort, but not urgent. Track as technical debt.

### Add timeout support to CLI flow execution

**Observation:** Daemon execution has 30-minute step timeout with process tree cleanup. CLI execution has no timeout—a stuck agent runs forever.

**Cost:** Medium. Port `StepTimeoutError` handling from runner.py to flow.py.

**Benefit:** Prevents runaway agents in CLI use.

**Verdict:** Worth adding—improves reliability.
