# Research: loopflow

## System understanding

Loopflow is a CLI-first orchestration layer for coding agents. Prompts are stored as versioned markdown steps; flows chain steps into DAGs; waves run flows continuously or on triggers. There are two main execution contexts: the interactive CLI (`lf`) and the daemon (`lfd`) that coordinates long-running waves with persistence, scheduling, and event streaming.

### Architecture

**CLI entry points (Typer):**

| Entry point | Package | Responsibility |
|-------------|---------|----------------|
| `lf` | `loopflow.lf` | Prompt launcher—assemble context, run steps/flows |
| `lfd` | `loopflow.lfd` | Daemon—manage waves, triggers, state, protocol |
| `lfops` | `loopflow.lfops` | Git workflow automation (PR, land, rebase, worktrees) |
| `lfwork` | `loopflow.lfwork` | Work queue integration (task sources) |

**Prompt/flow surface (`loopflow.lf`):**
- `cli.py` exposes `lf run`, `lf inline`, `lf flow` and step listing.
- `context.py` constructs `PromptComponents` (docs/diff/context/clipboard/directions) and token-trims before formatting.
- `flow.py` executes `Flow` DAGs (sequential, parallel, fork/synthesize, choose, loop-until-empty).
- `flows.py` parses YAML flows into `Step`, `Fork`, `Choose`, `LoopUntilEmpty`.
- `launcher.py` builds backend-specific commands (Claude/Codex/Gemini).
- `step.py` is the single-step execution entry point.

**Daemon surface (`loopflow.lfd`):**
- `daemon/server.py` provides the Unix socket protocol and pub/sub event broadcast.
- `daemon/http_server.py` exposes FastAPI REST endpoints plus JSON-compatible `/v1/*` for proto parity.
- `daemon/grpc_server.py` serves the gRPC API (proto-first control plane).
- `models.py` defines Wave/FlowRun/StepRun data models and status enums.
- `wave.py`, `flow_run.py`, `step_run.py` handle persistence to SQLite and operational state.
- `execution/runner.py` executes wave iterations (collector subprocess, timeouts, fork/synthesize, PR creation).

**Protocol layer:**
- Proto files live in `proto/` and generated Python in `src/loopflow/proto/...`.
- gRPC is the primary transport; JSON-over-HTTP v1 mirrors the proto structures for compatibility.
- The Unix socket protocol is JSON-over-newline with request/response + event streams.

### Data flow

**Single step (`lf <step>`):**
1. `gather_prompt_components()` assembles docs, diffs, clipboard, directions, summaries.
2. `trim_prompt_components()` enforces token budget and records dropped components.
3. `format_prompt()` creates the tagged XML prompt.
4. `build_model_command()` spawns the agent via `lfd.execution.collector` to capture output, autocommit, and step-run history.

**Flow execution (`lf flow <name>`):**
1. `load_flow()` parses YAML into `FlowItem`s.
2. `build_step_dag()` and `topological_batches()` schedule steps for sequential or parallel execution.
3. `run_flow()` handles Step/Fork/Choose/LoopUntilEmpty branching in one loop.
4. Forks spawn worktrees, run steps in parallel, then synthesize results back into the main worktree.

**Wave lifecycle (`lfd`):**
1. `lfd create` stores a `Wave` (flow + direction + area + stimulus) in SQLite.
2. `daemon/server.py` periodic checks evaluate watch/cron/loop triggers and manager slots.
3. `execution/runner.py` runs an iteration in a persistent worktree branch (`{wave}.main`), logging `FlowRun` + `StepRun` records.
4. Collector subprocess handles autocommit and optional PR creation; events are emitted to socket/HTTP clients.

### Key abstractions

- **Wave** (`lfd.models.Wave`): long-lived orchestration unit with `area`, `direction`, `flow`, and `stimulus` (once/loop/watch/cron). Owns a persistent worktree and main branch.
- **Flow** (`lf.flows.Flow`): DAG of `FlowItem`s; steps can specify `after`, `direction`, `model`, `interactive` overrides.
- **Fork/Choose/LoopUntilEmpty**: flow constructs for parallel drafts, decision branches, and backpressure loops.
- **PromptComponents** (`lf.context`): composable context parts; trimming logic enforces token budgets and tracks dropped elements.
- **FlowRun/StepRun** (`lfd.models`): execution records for daemon runs; socket/HTTP APIs surface these for clients.
- **Stimulus** (`lfd.models.Stimulus`): trigger configuration for waves (loop/watch/cron) with cron metadata.

## Tensions

- **Dual execution paths**: `lf/flow.py` and `lfd/execution/runner.py` both implement flow orchestration, fork handling, and worktree management, but with different logging/timeout/collector paths.
- **Transport parity**: socket, HTTP v1, and gRPC surfaces need to stay aligned; JSON compatibility endpoints mirror proto structs but the socket protocol has its own shape.
- **Prompt assembly complexity**: `context.py` centralizes many concerns (diff modes, summaries, wave context, clipboard images), which makes it a hotspot for feature accretion.
- **Wave branch naming**: daemon worktrees use `{wave}.main`, coupling naming to wave identity while `lfops wt` and `worktrunk` handle branching semantics differently.

## Observations

### Complexity

- **Flow branching is dense**: `run_flow()` handles Step, Fork, Choose, LoopUntilEmpty, and interactive execution in one loop with multiple special cases.
- **Runner orchestration is broad**: `execution/runner.py` combines prompt assembly, collector execution, timeouts, fork synthesis, PR creation, and daemon status updates.
- **Daemon periodic checks**: `daemon/server.py:_periodic_check()` mixes process cleanup, watch/cron checks, PR polling, and autoprune in one timed loop.
- **Worktree state tracking**: `lfd/worktree_state.py` and `lfops/worktrees.py` maintain multiple heuristics for merge and staleness detection.

### Quality

- **Tests exist but miss execution paths**: `tests/test_flows.py` focuses on parsing/topological batching but does not cover fork/synthesize execution or collector paths.
- **Error handling is mixed**: some loaders return `None` for not-found (steps, flows), while others raise `ValueError` for invalid structure.
- **Docs are strong at repo level**: `CLAUDE.md`, `STYLE.md`, `PROMPT_STYLE.md`, and `TESTING.md` give clear usage and style constraints.

### Potential

- **Protocol-first design enables clients**: gRPC + JSON v1 endpoints make it feasible for Swift/Concerto to integrate without socket parsing.
- **Flow DAGs allow real parallelism**: topological batching + forks are a strong foundation for multi-agent workflows.
- **Persistent wave worktrees**: long-lived worktrees simplify iteration continuity and PR evolution if merge detection stays reliable.

## Open questions

- Is `lfd/execution/worker.py` still part of the runtime path, or legacy code that can be removed?
- Is there an explicit parity contract between socket events, HTTP v1, and gRPC responses (especially for StepRun and FlowRun lifecycles)?
- Does the new `/health` endpoint in `daemon/http_server.py` supersede the TODO noted in reliability docs, or is there a separate health path planned for the socket server?
- Are `Choose` steps intended to bypass the collector (for speed), or should they follow the same step-run logging/timeout path as normal steps?

## Recommendations

### Document context trimming order
**Observation**: `trim_prompt_components()` drops components with a defined priority but the rationale is undocumented.
**Cost**: Low (comments only).
**Benefit**: Easier to reason about missing context and user-visible prompt truncation.
**Verdict**: Worth doing.

### Clarify or remove legacy execution code
**Observation**: `lfd/execution/worker.py` exists but `runner.py` appears to be the active entry point.
**Cost**: Low (investigate + delete or document).
**Benefit**: Reduces confusion over the true execution path.
**Verdict**: Worth doing after confirming runtime usage.

### Add fork/synthesize execution tests
**Observation**: Tests validate flow parsing but not fork/synthesize execution behavior.
**Cost**: Medium (mock worktrees/collector subprocesses).
**Benefit**: Protects the most distinctive orchestration feature from regressions.
**Verdict**: Worth doing when test harnesses can isolate side effects.

### Consolidate flow execution paths (future)
**Observation**: Flow orchestration exists in both `lf/flow.py` and `lfd/execution/runner.py`.
**Cost**: High (refactor, parity, regression risk).
**Benefit**: Single source of truth for flow semantics.
**Verdict**: Defer unless execution semantics need major change.
