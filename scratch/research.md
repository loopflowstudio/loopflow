# Research: loopflow.lfd-cli

## System understanding

Loopflow is a multi-surface CLI suite for orchestrating coding-agent work. The Python package ships four entry points: `lf` (prompt launcher), `lfd` (daemon for waves), `lfops` (git workflow tooling), and `lfwork` (work queue integration). Prompts are files (steps) and are composed with context (docs, diffs, clipboard, summaries) before being passed to external agent CLIs (Claude Code, Codex, Gemini). The daemon persists wave configuration in SQLite and runs iterations via worktrees, streaming progress over a Unix socket, HTTP, and gRPC.

### Architecture

- **Core CLI (`src/loopflow/lf/`)**
  - `context.py` gathers docs, diffs, summaries, directions, and clipboard into `PromptComponents`, trims to token budget, and formats the final prompt.
  - `step.py` implements `lf` commands, builds prompt/context, chooses model backend, and executes the agent CLI.
  - `flow.py` executes multi-step flows (linear, fork, choose, loop) and logs step runs via the daemon’s step-run store.
  - `launcher.py` translates model selection into concrete CLI commands and runner setup.
  - `flows.py` defines flow item types (Step, Fork, Choose, LoopUntilEmpty) and DAG building.
  - `directions.py`, `frontmatter.py`, `skills.py` load prompt directives and skill-based steps.

- **Daemon (`src/loopflow/lfd/`)**
  - `daemon/server.py` is the async Unix socket server with event broadcast, scheduler slot management, and periodic checks.
  - `daemon/http_server.py` exposes JSON-over-HTTP endpoints; `daemon/grpc_server.py` exposes gRPC.
  - `models.py`, `wave.py`, `flow_run.py`, `step_run.py` define wave/iteration/step data and persistence.
  - `execution/runner.py` runs a wave iteration, creating worktrees, executing steps through the collector, and updating DB state.
  - `execution/collector.py` spawns agent CLI processes, captures output, and reports back via the socket.
  - `worktree_state.py`, `pr_poller.py`, `autoprune.py`, `draft_prs.py` manage worktree metadata, PR state, and cleanup.

- **Operations (`src/loopflow/lfops/`)**
  - Git workflow commands: PR creation, land, next, worktrees, rebase, summarize, doctor, and commit helpers.

- **Work queue (`src/loopflow/lfwork.py`)**
  - Integrates external work items (Asana) and provides CLI helpers for claim/release/approve.

- **Protocol & clients**
  - `proto/` and `src/loopflow/proto/` define protobuf schemas and generated bindings.
  - `swift/` contains Concerto UI code and shared models/tests.

### Data flow

1. **Prompt assembly (lf)**
   - `lf` entrypoint → `step.py` parses flags → `gather_prompt_components()` collects:
     - repo docs (`scratch/`, `roadmap/`, `*.md`), area-specific docs, directions
     - diff files or raw diff (configurable)
     - summaries (optional)
     - clipboard text/images
   - `trim_prompt_components()` drops components to meet token limit, then `format_prompt()` builds the final prompt.
   - `launcher.py` builds the agent command and executes it (interactive or auto) in the worktree.

2. **Flow execution (lf)**
   - `flow.py` loads `.lf/flows/*` definitions → builds DAG → executes steps in order/parallel.
   - Steps log start/end via `lfd.step_run` utilities even when running outside the daemon.

3. **Wave iteration (lfd)**
   - `lfd` CLI updates wave config in SQLite.
   - Daemon `server.py` runs periodic checks for watch/cron stimuli and auto-PR/autoprune.
   - `execution/runner.py` creates worktree, builds prompts, runs steps via `collector.py`, updates flow/step/wave status.
   - Collector streams output lines back to the daemon socket; server broadcasts to subscribers and UI clients.

### Key abstractions

- **Step**: Markdown prompt file with frontmatter; executed by agent CLI.
- **Flow**: Ordered or DAG-based chain of steps, can fork/choose/synthesize.
- **Direction**: Perspective or intent applied to steps (markdown files under `.lf/directions/`).
- **Wave**: Persistent configuration (area × direction × flow × stimulus) managed by `lfd`.
- **PromptComponents**: Structured context chunks before formatting.
- **StepRun / FlowRun**: Execution records stored in SQLite and streamed to clients.

## Tensions

- **Dual execution paths**: `lf flow` and `lfd` iteration both execute steps, but share only some logging paths. Divergence risks inconsistent behavior between manual and daemon runs.
- **Context trimming tradeoffs**: `trim_prompt_components()` drops content greedily after special-casing diff_files; ordering and rationale are implicit, which can surprise users when context is trimmed.
- **Protocol surfaces**: socket, HTTP, and gRPC all exist; keeping parity and behavior consistent across them is a recurring risk.

## Observations

### Complexity

- `src/loopflow/lf/context.py` handles gathering, budgets, and trimming logic in one module with multiple rules (diff vs files, area docs, summaries, clipboard).
- `src/loopflow/lf/flow.py` interleaves DAG execution, worktree management, runner selection, and step-run logging.
- `src/loopflow/lfd/daemon/server.py` mixes scheduling checks, PR polling, and event broadcast in one loop.

### Quality

- Test coverage is broad across CLI, flow logic, daemon protocol, and summarization (see `tests/test_*`).
- Error handling is generally user-facing with Typer exit codes; some background paths swallow exceptions (daemon periodic loop) to keep service alive.
- Documentation is thorough: CLI references, configuration, and daemon usage live under `docs/` and module READMEs.

### Potential

- The gRPC server exists alongside HTTP and socket; it’s positioned for richer clients once parity is complete.
- The flow DAG supports fork/synthesize patterns and parallelism, but tests focus more on parsing than execution.
- Worktree state tracking, PR polling, and autoprune provide a foundation for a richer live UI (Concerto).

## Open questions

- Is `src/loopflow/lfd/execution/worker.py` still used in production paths, or is it legacy relative to `runner.py`?
- How intentionally different are `lf flow` execution semantics versus `lfd` iterations (e.g., step logging, collector usage)?
- Are there explicit guarantees about parity between socket, HTTP, and gRPC behaviors?

## Recommendations

### Document context trimming strategy
**Observation**: `trim_prompt_components()` uses special-case dropping for diff files plus greedy dropping for others; the rationale isn’t documented near the code.
**Cost**: Low (comment block).
**Benefit**: Maintainers can reason about trimming behavior and adjust priorities intentionally.
**Verdict**: Worth it; behavior is user-visible when prompts get trimmed.

### Add execution-path parity tests for fork/synthesize
**Observation**: Flow parsing is tested, but fork/synthesize execution paths aren’t directly exercised in `tests/`.
**Cost**: Medium (mock worktree/runner/collector).
**Benefit**: Protects a key feature from regressions and clarifies expected behavior.
**Verdict**: Worth it if fork/synthesize is a flagship workflow.
