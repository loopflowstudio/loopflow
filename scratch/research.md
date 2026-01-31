# Research: Loopflow Codebase

## System Understanding

### Entry Points

Three CLIs serve as entry points, all defined in `pyproject.toml`:

| CLI | Module | Purpose |
|-----|--------|---------|
| `lf` | `loopflow.lf.cli:main` | Prompt launcher and step/flow runner |
| `lfd` | `loopflow.lfd.cli:main` | Daemon for orchestrating waves |
| `lfops` | `loopflow.lf.cli:ops` | Git workflow operations (branch, PR, land) |

**Happy Path (Single Step)**:
```
lf design -c swift/
  → cli.py rewrites to `lf run design -c swift/`
  → step.py loads step, gathers context
  → context.py assembles docs + diff + clipboard + step into prompt
  → execution.py builds command, launches backend
  → launcher.py spawns `claude --print` with prompt
```

**Happy Path (Wave Execution)**:
```
lfd watch swift/ → creates Wave + Stimulus(kind=watch)
  → daemon/server.py runs periodic check_stimuli()
  → wave.py detects main branch change, queues activation
  → execution/runner.py creates worktree, runs flow steps
  → creates PR targeting wave's main branch
```

### Core Abstractions

**Data Structures** (`models.py`):

| Type | Purpose |
|------|---------|
| `Wave` | Orchestrated unit of work: area × direction × flow |
| `Stimulus` | Trigger config (once/loop/watch/cron) pointing to a wave |
| `FlowRun` | Execution instance of a flow, spawned by a wave |
| `StepRun` | Single step execution (can be standalone or part of flow) |
| `Flow` | DAG of steps loaded from YAML |
| `Step` | Named step with optional model override and dependencies |

**Flow Control Types** (`flows.py`):
- `Fork`: Parallel execution with synthesis
- `Choose`: Runtime branch selection
- `LoopUntilEmpty`: Iterate until condition met

### Module Responsibilities

| Module | Responsibility |
|--------|----------------|
| `lf/context.py` | Context assembly: gather docs, diff, clipboard → `PromptComponents` |
| `lf/execution.py` | Step execution: `ExecutionParams` → spawn backend → capture result |
| `lf/launcher.py` | Backend portability: build commands for Claude/Codex/Gemini |
| `lf/flows.py` | Flow loading: YAML → `Flow` with `Step`/`Fork`/`Choose` items |
| `lf/flow.py` | Flow execution: topological ordering, fork synthesis |
| `lfd/wave.py` | Wave persistence: CRUD, stimulus checking, activation logic |
| `lfd/db.py` | SQLite infrastructure: migrations, connection management |
| `lfd/daemon/server.py` | Asyncio server: request dispatch, event broadcast, periodic checks |
| `lfd/execution/runner.py` | Iteration execution: worktree → steps → PR creation |

### Data Flow

**Context Assembly Pipeline** (`context.py:gather_prompt_components`):
```
repo_root
  ├─ docs: scratch/*.md + reports/*.md + root *.md files
  ├─ diff_files: git diff or explicit paths
  ├─ clipboard: optional pasted content
  ├─ direction: resolved .lf/directions/*.md files
  ├─ step: .lf/steps/<name>.md or builtin
  └─ loopflow_doc: bundled system documentation

→ format_prompt() → XML-tagged sections:
  <lf:docs>...</lf:docs>
  <lf:files>...</lf:files>
  <lf:direction:X>...</lf:direction:X>
  <lf:step:Y>...</lf:step:Y>
```

**Wave Lifecycle**:
```
Wave.paused=True (created)
  ↓ lfd start/unpause
Wave.paused=False + Stimulus attached
  ↓ check_stimuli() → should_activate_*()
PendingActivation queued
  ↓ consume_pending_activations()
run_iteration() → FlowRun
  ↓ step by step execution
PR created, merged, FlowRun.status=COMPLETED
  ↓ iteration++
Back to stimulus checking
```

**Daemon Event System**:
- `notify_event()` - fire-and-forget notifications to daemon
- `DaemonClient.subscribe()` - async iterator for event streaming
- Events: `wave.started`, `wave.step.completed`, `wave.waiting`, etc.

### Backend Portability

`launcher.py` provides unified interface for three backends:

| Backend | Command Builder | Notes |
|---------|-----------------|-------|
| Claude | `build_claude_command()` | `--print --dangerously-skip-permissions` for auto |
| Codex | `build_codex_command()` | `--sandbox workspace-write`, `-i` for images |
| Gemini | `build_gemini_command()` | `--output-format stream-json`, `--yolo` |

Each backend has:
- Auto mode: batch execution with JSON streaming
- Interactive mode: terminal UI handoff

## Tensions

### 1. Wave vs Direct Execution

Two execution paths exist:
- `lf run <step>`: Direct, single-step, uses `execution.py`
- `lfd` waves: Orchestrated, multi-step, uses `execution/runner.py`

Some duplication in prompt building (`_build_loop_prompt` vs `gather_prompt_components`) and execution logic. The runner has its own context assembly that wraps but doesn't fully reuse `context.py`.

### 2. Step Discovery Complexity

Steps can come from 5 sources with precedence:
1. `.lf/steps/<name>.md` (user, repo-local)
2. `.claude/commands/<name>.md` (legacy location)
3. Global step paths (external skill sources)
4. Builtin steps (bundled in package)
5. External skills (from config)

`list_all_steps()` returns 4 separate lists. Discovery logic spread across `context.py`, `skills.py`.

### 3. Interactive vs Auto Mode

Flows can contain interactive steps (`interactive: true` in frontmatter). The `tick_flow()` state machine handles pausing at interactive steps, but the pause/resume flow is complex:
- Step marked WAITING
- Wave status → WAITING
- User completes step externally
- `continue_step_run()` advances `step_index`
- `tick_flow()` resumes

### 4. Database Schema Coupling

`models.py` has `wave_from_row()` / `stimulus_from_row()` functions that manually map dict → model. Pydantic could do this, but the custom converters handle type coercion (JSON arrays, timestamps). Migration system in `lfd/migrations/` suggests schema evolves frequently.

## Observations

### Complexity

**Intentional complexity** (worth the cost):
- Fork/Choose/LoopUntilEmpty flow control enables sophisticated orchestration
- Backend portability (Claude/Codex/Gemini) requires per-backend command builders
- Stimulus system (once/loop/watch/cron) gives flexible trigger options

**Potentially reducible**:
- Wave model has 20+ fields including both config and runtime state
- Context assembly has many feature flags (diff_mode, clipboard, summaries)
- Parallel step execution in `runner.py` duplicates worktree creation logic

### Quality

**Well-tested areas**:
- `test_context.py`: 50+ tests covering context assembly, templates, trimming
- `test_flows.py`: Flow parsing, DAG building, topological ordering
- `test_directions.py`, `test_frontmatter.py`: Edge cases covered

**Test patterns**:
- Heavy use of `tmp_path` fixture for isolated repos
- Mocking for subprocess calls, git operations
- No integration tests hitting real LLM backends

**Areas with less coverage**:
- `execution/runner.py` - complex orchestration logic
- `daemon/server.py` - asyncio event handling
- Wave lifecycle state transitions

### Potential

**Extension points**:
- New backends via `Runner` ABC in `launcher.py`
- New stimuli kinds via `StimulusKind` enum + `should_activate_*` functions
- New flow control via `flows.py` types (Fork, Choose already exist)

**Simplification opportunities**:
- Wave could split into `WaveConfig` (immutable) + `WaveState` (runtime)
- Context assembly could be a proper pipeline with typed stages
- Step discovery could unify into a single search path with priority

## Open Questions

1. **Squash merge recovery**: `base_commit` field on Wave is for squash merge recovery - what's the failure mode it addresses?

2. **PR limit enforcement**: Wave has `pr_limit: int = 5` - how is this enforced? I see it defined but didn't trace the enforcement logic.

3. **Stacking support**: `base_branch` and `base_commit` suggest PR stacking - is this actively used? The `lfd next` command references it.

4. **Summary caching**: `db.py` has `save_summary_db` / `load_summary_db` for caching summaries - what generates these summaries?

5. **Collector subprocess**: `execution/collector.py` is referenced but not read - what does it capture beyond exit codes?
