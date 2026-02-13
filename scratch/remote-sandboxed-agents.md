# 01: Sandboxed Agent Execution

Sandbox agent step execution in `lfd` so host subprocesses are no longer the only runtime model.

## Current state

This branch shipped the execution abstraction and Docker backend:

- `AgentExecutor` with `LocalProcessExecutor` and `DockerExecutor`
- Executor selection from config/env (`executor.type`, `executor.image`, env overrides)
- Ephemeral Docker container per step with log streaming and explicit cleanup
- Executor-aware stop/delete/recovery termination paths
- Explicit credential injection (env + configured mounts) for Docker mode

Local execution remains the default.

## What remains to finish Phase 01

### 1) Repo-in-volume workspace model

Current Docker mode bind-mounts the active host worktree at `/workspace`.

Remaining work:
- Move clone/worktree lifecycle to Docker-managed per-repo volumes
- Mount only that repo volume into agent containers
- Ensure no broad host path mounts are required for normal operation

### 2) Durable container tracking

Current container IDs are in-memory only.

Remaining work:
- Persist active container IDs in run state/store
- Rehydrate terminate handles after daemon restart
- Verify stop/delete/recovery works across restarts

### 3) Credential mount UX

Current config uses raw `host_path:container_path` strings.

Remaining work:
- Decide whether to keep string syntax or switch to structured mount config
- Add clear validation/errors for malformed entries
- Keep deny-by-default behavior and read-only semantics

### 4) Runtime validation in live Docker environment

Current validation is unit tests + compile/test suite.

Remaining work:
- Add daemon-level end-to-end Docker execution test(s)
- Cover run, log streaming, cancellation, and cleanup behavior
- Validate behavior under concurrent waves

## Open decisions

- Do we enforce volume-backed workspace as a hard requirement for `executor.type: docker`, or allow bind-mount fallback behind explicit opt-in?
- Should credential mounts be promoted to a first-class typed config now, or deferred to Phase 02 with stricter parser/validation in Phase 01?

## Done criteria (Phase 01)

- Docker execution runs from Docker-managed repo volumes, not host worktree binds
- Cancellation/termination survives daemon restart (container tracking is durable)
- Credential config is explicit, validated, and least-privilege by default
- Live Docker end-to-end tests cover run + cancel + cleanup
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` pass
