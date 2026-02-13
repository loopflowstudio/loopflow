# Research: Remote Execution & Fork Hardening (01C Branch)

## System understanding

Loopflow's execution stack has three clean layers with one-way dependencies:

```
engine/  (shared library: data models, flow loading, agent commands, worktree ops)
   ↑              ↑
   │              │
lf/            lfd/
(CLI)          (daemon)
```

Neither CLI nor daemon imports the other. Both consume the same `engine::` types (`Flow`, `ConcreteItem`, `FlowAction`, `ForkSelect`, `Config`). This separation is well-maintained.

### Architecture

**engine/** — Pure data structures and algorithms. Loads flows from YAML, expands them into `ConcreteItem` sequences (steps + forks), and provides `next_action()` to drive execution. Also handles worktree creation/removal, agent command building for 4 backends (Claude, Codex, Gemini, OpenCode), and config loading/merging.

**lf/commands/flow.rs** — CLI flow runner. Synchronous, thread-based. Creates sibling worktrees for fork branches, spawns OS threads for parallel execution, writes a manifest to `.lf/fork-manifest.json`, and cleans up after synthesis. Only supports `ForkSelect::All`.

**lfd/executor.rs** — Daemon flow executor. Async (tokio), supports Docker and local process backends. Manages Docker volumes, shared clones, per-wave worktrees, image fingerprinting, container lifecycle, credential mounts, and startup recovery. Supports all three `ForkSelect` modes. This is the largest file in the codebase at 3700+ lines.

**lfd/config.rs** — Daemon-specific config. Loads from `~/.lf/lfd.yaml`, supports `local` and `docker` executor types, enforces an allowlist for credential mounts (claude, codex, gemini, gitconfig, ssh, gnupg).

### Data flow

A flow execution follows this path:

```
YAML (steps/*.md, flows/*.yaml)
  → load_flow() → expand_flow() → Vec<ConcreteItem>
  → next_action(items, index) → FlowAction
  → match FlowAction:
      RunStep    → build_agent_command() → subprocess or container
      Fork(All)  → create worktrees → parallel execution → synthesize → cleanup
      Complete   → finalize
```

Fork execution diverges between CLI and daemon:

| Concern | CLI (lf/) | Daemon (lfd/) |
|---------|-----------|---------------|
| Parallelism | OS threads | tokio tasks |
| Worktrees | Sibling dirs (`repo.fork-N`) | Docker volume worktrees |
| Persistence | Manifest JSON file | SQLite/Postgres store |
| Recovery | None | Container rehydration |
| Select modes | All only | All, One, Prompt |

### Key abstractions

**`AgentExecutor` trait** — The core abstraction for running agents. Two implementations: `LocalProcessExecutor` (direct subprocess) and `DockerExecutor` (container-based with volume management, image lifecycle, credential isolation, and startup recovery).

**`KeyedLocks`** — Custom per-key mutex manager in the Docker executor. Serializes mutations to the same shared clone (repo-level locks) and prevents concurrent duplicate image builds (image-level locks).

**`RepoVolumeIdentity`** — Deterministic naming for Docker volumes and images. SHA256 of canonical repo URL or local path. Prevents cross-repo collisions.

**`ForkManifest` / `ForkBranchTask`** — CLI-side fork tracking. Records branch metadata, worktree paths, and exit codes for downstream synthesis.

## Tensions

- **CLI vs daemon fork parity**: CLI supports only `ForkSelect::All` while the daemon supports all three modes. The CLI returns a hard error for One/Prompt. This is a deliberate constraint, not a bug — One/Prompt require interactive state or user prompting that the CLI's synchronous model doesn't support well. But it means flows that use `select: one` or `select: prompt` can only run via `lfd`.

- **Docker CLI shelling vs Bollard API**: Image builds shell out to `docker build` instead of using the Bollard API that's already a dependency for container management. This creates a runtime dependency on Docker CLI being in PATH. The Bollard crate supports image builds, so there's a split between managed (containers via API) and unmanaged (builds via CLI). This is an acknowledged open question in `scratch/questions.md`.

- **executor.rs size**: At 3700+ lines, this file combines volume management, container lifecycle, image fingerprinting, credential resolution, startup recovery, wave orchestration, fork execution, PR creation, branch advancement, and summary management. These are related but distinct responsibilities.

- **Synchronous context gathering in daemon**: `build_step_prompt()` calls `gather_context()` synchronously, blocking the executor thread. For large repos, this delays step launches. The rest of the daemon is fully async.

- **Fork cleanup is sequential**: Both CLI (`cleanup_fork_artifacts`) and daemon (`cleanup_fork`) remove worktrees one at a time. With many fork branches, this is unnecessarily slow.

## Observations

### Complexity

**executor.rs:1646-1680** (`prepare_workspace`) — The workspace preparation pipeline has 5 stages (ensure volume, shared clone, fetch, worktree, hygiene) with a mutation lock around the first three. The `prepared_key` tracking prevents redundant fetches but has no TTL or invalidation — a restarted run could use stale code if the remote changed between preparation and restart.

**executor.rs:604-640** (`plan_rehydration`) — Daemon restart recovery inspects every running agent, checks container state via Docker API, and classifies each as reattachable or lost. The logic is sound but the cascading state updates (agent → wave_run → wave) across multiple store calls make it hard to reason about consistency if any individual update fails. Current approach: warn and continue, which is correct for recovery.

**executor.rs:2407-2644** (`run_fork` in WaveExecutor) — Fork execution spawns concurrent tasks with scheduler-gated slot acquisition. On failure, it calls `cancel.cancel()` then `handle.abort()` on all tasks. The abort is not graceful — if a task is mid-execution, its `scheduler.release()` call in the task body may not run, potentially leaking a scheduler slot.

**lf/commands/flow.rs:100-200** (`run_fork` in CLI) — The CLI fork runner creates worktrees, spawns threads, and collects results through a channel. The fail-late policy (wait for all branches, then report aggregate errors) is good, but there's no mechanism to cancel running branches if one fails catastrophically.

### Quality

**Documentation**: The `docs/lfd.md` reference doc is well-structured and covers executor config, credential mounts, and Docker mode. It was updated alongside the code changes.

**Error messages**: Docker build failures now include context about Docker CLI availability (commit `87728824`). Fork branch failures now log exit codes (commit `af2acc6c`). The credential mount allowlist produces clear rejection messages for unknown mount names.

**Test coverage for new features**: The fork execution flow (`run_fork` in both CLI and daemon) has unit tests for helper functions (worktree path naming, manifest serialization, direction merging, credential mount resolution) but no integration tests for the end-to-end fork execution path. The most critical new code path — parallel worktree creation, thread/task spawning, result aggregation, and cleanup on failure — is untested.

**Inline tests in config.rs**: Good coverage of YAML deserialization, env overrides, invalid config fallback, and credential mount rejection. These test behavior, not wiring.

**Existing test suite**: 22 config tests, 9 wave worktree tests, 8 flow tests, 7 worktree tests, 5 agent tests. The existing tests are well-written — they test behavior, use real git repos via tempdir, and avoid mock-wiring assertions.

### Potential

**Executor trait extension**: The `AgentExecutor` trait is clean and could support additional backends (EC2, remote SSH, Kubernetes) without changing the orchestration layer. The roadmap already has items for EC2 (`roadmap/remote/04-ec2.md`).

**Fork select modes in CLI**: Adding `ForkSelect::One` to the CLI would be straightforward — pick the first branch (or prompt in interactive mode) and skip the parallel machinery. `ForkSelect::Prompt` would require a TUI picker but the direction list is already available.

**Image fingerprint as cache key**: The SHA256-based fingerprint for Dockerfile + env-setup + base image is a good foundation for more sophisticated caching — e.g., caching intermediate build layers across repos that share the same base image.

**Sequential → parallel cleanup**: Both CLI and daemon remove fork worktrees sequentially. These operations are independent and could run concurrently.

## Open questions

- No timeout mechanism exists for agent execution (containers or processes). How should runaway agents be handled? Is this an acceptable risk at this stage, or should a configurable timeout be added?
- The `prepared_runs` cache in DockerExecutor tracks which workspaces have been prepared for a given run. If a run is interrupted and restarted, the cache prevents re-fetching. Is stale code an acceptable trade-off, or should workspace preparation always re-fetch?
- Fork task abort in the daemon (`handle.abort()`) can leak scheduler slots if the task hasn't reached its cleanup code. Should tasks use `CancellationToken` checks instead of hard aborts?
- executor.rs at 3700+ lines could be split into focused modules (volume management, container lifecycle, wave orchestration, recovery). Is this worth the churn now, or should it wait until the next major feature addition?

## Recommendations

### 1. Add integration tests for fork execution
**Observation**: The parallel fork execution path in both CLI and daemon has unit tests for helpers but no integration tests for the end-to-end flow (create worktrees → run branches → aggregate results → cleanup).
**Cost**: Medium — requires a test harness that can create temporary repos with flows containing forks. The existing `TestRepo` setup in `tests/support/` provides the foundation.
**Benefit**: High — fork execution is the main new feature on this branch. Failure scenarios (one branch fails, cleanup after manifest write failure) are especially important to cover.
**Verdict**: Worth it. The existing test infrastructure supports this. Focus on: (1) successful fork with 2+ branches, (2) partial failure with cleanup, (3) direction merging through the fork path.

### 2. Add Docker CI coverage
**Observation**: Already identified in `scratch/remote-01c-hardening.md` as remaining work for 01C closeout. Docker-specific tests (`cargo test -- --ignored`, `test_docker_smoke.sh`) exist but aren't wired into CI.
**Cost**: Low — the tests exist; the CI config just needs updating. Nightly schedule for the heavier Docker e2e suite.
**Benefit**: High — Docker executor changes have no automated regression coverage in CI.
**Verdict**: Worth it. This is blocking 01C closeout.

### 3. Split executor.rs into focused modules
**Observation**: 3700+ lines combining volume management, container lifecycle, image fingerprinting, credential resolution, startup recovery, wave orchestration, fork execution, PR creation, and summary management.
**Cost**: Medium — pure refactoring, no behavior change. Risk of merge conflicts if other work is in flight.
**Benefit**: Medium — improves navigability and allows targeted review of specific concerns. Makes it easier to add new executor backends (EC2, K8s) without growing the file further.
**Verdict**: Worth doing, but not urgent. The code is well-organized internally with clear section boundaries. Schedule this before adding the next executor backend.
