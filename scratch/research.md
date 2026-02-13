# Research: Loopflow Execution Stack

## System understanding

Loopflow is a multi-language system for orchestrating coding agents at scale. Three clean layers:

```
engine/  (shared library: flow loading, agent commands, context assembly, worktree ops)
   ↑              ↑
   │              │
lf/            lfd/
(CLI)          (daemon)
```

Neither CLI nor daemon imports the other. Both consume `engine::` types. A Python client (`lfq`) and Swift macOS app (Concerto) talk to the daemon via REST API.

### Architecture

**engine/** (14 modules, ~225K lines total) — Pure data structures and algorithms. Core responsibilities:
- **flow.rs** — Load YAML flow definitions, parse into `Flow`/`FlowItem` trees, expand into `ConcreteItem` sequences, drive execution via `next_action()`
- **fork.rs** — Fork utilities: worktree path naming, direction merging, manifest serialization, cleanup
- **prompt.rs** — Context assembly: gather step/direction/diff/clipboard/area docs, token counting (tiktoken), budget-based trimming with eviction priority
- **agent.rs** — Command building for 4 agent backends (Claude, Codex, Gemini, OpenCode), process management with SIGTERM on Ctrl+C
- **stream.rs** — Parse streaming JSON output from all 4 agents into unified `StreamEvent` types
- **config.rs** — Layered config (global + repo YAML), model parsing, context budget defaults
- **worktree.rs / worktrees.rs** — Git worktree lifecycle, branch naming, cleanup with main-repo resolution
- **builtins.rs** — `include_str!()` embedded steps (18), flows (12), directions (7), ops prompts (5)

**lf/** — CLI entry point. Clap-based argument parsing with smart flag reordering. Commands: `run` (step execution), `flow` (pipeline execution), `ops` (git operations), `list`, `shell`.

**lfd/** — Daemon. Axum HTTP server + SQLite/Postgres persistence + scheduler with slot-based concurrency. Background triggers: loop (5s), watch (30s), cron (30s), recovery, summary refresh. REST API at `/v0/` with WebSocket streaming.

### Data flow

```
YAML (steps/*.md, flows/*.yaml)
  → load_flow() → expand_flow() → Vec<ConcreteItem>
  → next_action(items, index) → FlowAction
  → match FlowAction:
      RunStep      → gather_context() → trim → format_prompt() → agent subprocess/container
      Fork(All)    → create worktrees → parallel execution → synthesize → cleanup
      WaitInteract → pause for user → then RunStep
      Complete     → finalize
```

Context assembly is the critical path: loads step content, directions, repo docs (scratch/, roadmap/, root .md), diff (auto-tiered: full unified if <15k tokens, else stat-only), area docs (parent READMEs), clipboard, and LOOPFLOW.md system docs. Trims to budget with eviction order: area → summaries → docs → diff files → diff → clipboard.

### Key abstractions

**Flow model**: `Flow` → `Vec<FlowItem>` where items are `Step`, `Fork { branches, select }`, or `FlowRef(name)`. Flows expand recursively (max depth 5 with cycle detection) into `Vec<ConcreteItem>` with parent chain tracking for display paths.

**AgentExecutor trait**: Core abstraction for running agents. Methods: `run()`, `terminate()`, `recover_startup()`, `cleanup_wave()`. Two implementations: `LocalProcessExecutor` (subprocess) and `DockerExecutor` (containers with volume management, image lifecycle, credential isolation).

**Config layering**: Global (`~/.lf/config.yaml`) + repo (`.lf/config.yaml`). Scalars override, additive keys combine. Daemon has separate config (`~/.lf/lfd.yaml`) for executor type, auth, credentials.

**KeyedLocks**: Custom per-key mutex in Docker executor. Serializes mutations to shared clones (repo-level) and prevents concurrent duplicate image builds (image-level).

## Tensions

- **CLI vs daemon fork parity**: CLI supports only `ForkSelect::All`; daemon supports All, One, and Prompt. One/Prompt require interactive state the CLI's synchronous model doesn't support. Flows using `select: one` or `select: prompt` only work via lfd.

- **Docker CLI shelling vs Bollard API**: Image builds shell out to `docker build` while container management uses the Bollard API. Split between managed (containers via API) and unmanaged (builds via CLI). Runtime dependency on Docker CLI in PATH.

- **Synchronous context gathering in async daemon**: `gather_context()` blocks the executor thread. For large repos, this delays step launches. The rest of the daemon is fully async.

- **executor.rs god file**: 3693 lines combining volume management, container lifecycle, image fingerprinting, credential resolution, startup recovery, wave orchestration, fork execution, PR creation, branch advancement, and summary management. Roadmap explicitly calls out splitting this.

- **Synthesize step hardcoded**: Forks always run a step named `"synthesize"` after branches complete. Previously this was configurable via `fork.synthesize`. Flows with custom synthesis step names break silently.

## Observations

### Complexity

**executor.rs `prepare_workspace`** — 5-stage pipeline (ensure volume, shared clone, fetch, worktree, hygiene) with mutation lock around first three. The `prepared_key` tracking prevents redundant fetches but has no TTL — a restarted run could use stale code if the remote changed between preparation and restart.

**executor.rs `run_fork` (daemon)** — Fork execution spawns concurrent tasks with scheduler-gated slot acquisition. On failure, `cancel.cancel()` then `handle.abort()` on all tasks. The abort is not graceful — if a task is mid-execution, `scheduler.release()` in the task body may not run, potentially leaking a scheduler slot.

**executor.rs `plan_rehydration`** — Daemon restart recovery inspects every running agent, checks container state via Docker API, classifies each as reattachable or lost. Cascading state updates (agent → wave_run → wave) across multiple store calls. Current approach: warn and continue, which is correct for recovery.

**flow.rs expansion** — Plain string items that match sub-flow names are expanded as sub-flows, not steps. Step takes priority over flow for ambiguous names. This was the fix for `step not found: publish` in roadmap-reduce. Well-tested with 3 dedicated test cases.

### Quality

**Test coverage**: 253 `#[test]` functions across 22 source files, plus 17 integration test files. Tests use real git repos (tempdir), assert on behavior not mock wiring, and cover edge cases well. Flow expansion tests are particularly thorough — 7 cases covering parsing, nesting, cycle detection, and ambiguous name resolution.

**Test gaps**: No integration tests for end-to-end fork execution (parallel worktree creation, thread/task spawning, result aggregation, cleanup on failure). The most critical new code path is untested. Fork unit tests cover helpers (path naming, manifest, direction merging) but not the orchestration.

**Error messages**: Docker build failures include context about Docker CLI availability. Fork branch failures log exit codes. Credential mount allowlist produces clear rejection messages. Worktree removal errors are logged but don't propagate — orphaned worktrees can accumulate silently.

**Documentation**: `docs/lfd.md` and `roadmap/remote/01-sandboxed-agents.md` are thorough and current. `reports/execution-architecture.md` provides a useful architecture overview. Code-level documentation is minimal but types are expressive enough to be self-documenting.

**Patterns**: Consistent use of `thiserror` for library errors, `anyhow` for application-level errors. `From` conversions between error types follow a clear hierarchy. Derives follow style guide (`Debug` on all public types). Config parsing uses serde with sensible defaults.

### Potential

**Executor trait extensibility**: Clean abstraction with minimal surface area (4 methods). Adding EC2, SSH, or Kubernetes backends would require only new `AgentExecutor` implementations without changing orchestration.

**Fork select modes in CLI**: `ForkSelect::One` could be added trivially — pick branch 0 (or prompt in interactive mode) and skip parallel machinery. `ForkSelect::Prompt` needs a TUI picker but direction data is available.

**Sequential → parallel cleanup**: Both CLI and daemon remove fork worktrees one at a time. These are independent operations and could run concurrently (rayon for CLI, tokio::join for daemon).

**Image fingerprint as cache layer**: SHA256-based fingerprint for Dockerfile + env-setup + base image is a good foundation for shared caching across repos with identical base images.

**Context assembly optimization**: `gather_context()` could be made async or run on a blocking thread pool (tokio::spawn_blocking) to avoid blocking the daemon's async runtime.

## Open questions

- Should `ForkSelect::One` and `ForkSelect::Prompt` ever be supported in CLI mode, or is the daemon the right boundary for interactive/selective fork execution?
- Is the `prepared_key` tracking in workspace preparation sufficient without a TTL? Could a long-running daemon serve stale code after remote pushes between runs?
- When fork cleanup partially fails (some worktrees removed, others not), should there be a recovery mechanism to detect and clean orphaned fork worktrees on next daemon startup?
- The scheduler slot leak on task abort during fork failure — is this a real issue in practice, or does the daemon's recovery loop handle it?

## Recommendations

### Split executor.rs into focused modules
**Observation**: 3693 lines mixing 10+ distinct responsibilities. The roadmap explicitly identifies this as pre-work for the EC2 executor (phase 04).
**Cost**: Medium. Well-defined boundaries between volume management, container lifecycle, image building, credential resolution, wave orchestration, fork execution, and PR/branch management. Mostly mechanical extraction.
**Benefit**: Each module becomes independently testable. New executor backends only need to touch their own module. Code review becomes tractable.
**Verdict**: Worth it. Do before adding the next executor backend.

### Add fork execution integration tests
**Observation**: The most critical new code path (parallel worktree creation → thread/task spawning → result aggregation → cleanup on failure) has zero integration test coverage. Unit tests cover helpers only.
**Cost**: Low-medium. The `TestRepo` harness in `rust/loopflow-test-support/` provides the foundation. Need to set up flows with forks, create real worktrees, and verify manifest + cleanup.
**Benefit**: Catches regressions in the orchestration layer that unit tests miss entirely. Fork execution involves concurrency, I/O, and cleanup — exactly the category where bugs hide.
**Verdict**: High priority. The fork execution path is new, concurrent, and has real failure modes (partial worktree creation, thread panics, cleanup races).

### Make context gathering non-blocking in daemon
**Observation**: `gather_context()` does synchronous filesystem I/O and token counting. In the async daemon, this blocks the executor thread.
**Cost**: Low. Wrap the existing sync call in `tokio::spawn_blocking()`. No architectural change needed.
**Benefit**: Prevents step launch delays for large repos. Keeps the async runtime responsive.
**Verdict**: Worth it as a small follow-up. Straightforward change with clear benefit.
