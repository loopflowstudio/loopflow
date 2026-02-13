# Gate review: sandboxed agent executor (docker backend)

## What was implemented
- Added a pluggable agent execution layer in `lfd` with `AgentExecutor`.
- Split execution into:
  - `LocalProcessExecutor` (existing subprocess behavior, now with explicit terminate support).
  - `DockerExecutor` (ephemeral one-container-per-step execution via Docker API / `bollard`).
- Wired executor selection from `~/.lf/lfd.yaml` (`executor.type`, `executor.image`, `credentials`) plus env overrides (`LFD_EXECUTOR_TYPE`, `LFD_EXECUTOR_IMAGE`).
- Integrated executor-aware termination in wave stop/delete and stuck-run recovery paths.
- Added Docker log streaming into existing `OutputHub` parsing pipeline.
- Added docs in `docs/lfd.md` for daemon executor configuration.

## Key choices
- **Typed Docker API (`bollard`) instead of shelling out**: keeps lifecycle operations structured and async-native.
- **Ephemeral per-step containers**: minimizes state bleed across steps and simplifies cleanup semantics.
- **Deny-by-default credentials**: only explicitly listed env vars/mounts are injected.
- **Gate polish updates**:
  - credential mount host paths now expand `~/...` so documented config works in practice,
  - invalid executor env type values no longer silently force local mode,
  - command path rewriting now avoids false-positive prefix rewrites (`/tmp/worktree-copy` no longer becomes `/workspace-copy`).

## How it fits together
`WaveExecutor` delegates step launch/termination to an `AgentExecutor` chosen at daemon boot from `LfdConfig`. Runtime paths that need process control (HTTP stop/delete and recovery loop) call `WaveExecutor::terminate_agent`, which routes to local PID kill or Docker container stop/remove. Output from either backend converges through the same stream parsing and `OutputHub` event flow.

## Risks and bottlenecks
- Docker mode still bind-mounts the active worktree path; full per-repo Docker volume lifecycle is not yet implemented.
- Active Docker container IDs are in-memory only; daemon restarts can lose direct stop handles for already-running containers.
- Credential mount syntax remains raw `host:container` strings; validation is stricter now, but UX could improve with structured config.
- Docker runtime behavior is covered by unit tests and compile/test suites, but not by a live daemon+docker end-to-end test in this branch.

## What's not included
- No compose stack / remote deployment changes (Phase 02+ scope).
- No persistent container-id storage model.
- No network policy engine or egress allowlisting.
- No migration to Docker-managed workspace volumes yet.
