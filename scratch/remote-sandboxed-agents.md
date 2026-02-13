# Remote sandboxed agents

## Current state (commit 1)

Docker executor now runs from persistent per-repo Docker volumes instead of host worktree bind mounts.

### Implemented foundation

- Persistent repo volume identity:
  - canonicalized `remote.origin.url` when available
  - fallback hash of absolute local repo path
- Git layout inside volume:
  - shared clone per repo
  - per-wave worktrees in the same volume
- Reproducible pre-run hygiene in Docker mode:
  - `git fetch`
  - `git reset --hard <target>`
  - `git clean -fdx`
- Fine-grained locking around shared clone mutations:
  - clone/fetch/ref updates/worktree add-remove are serialized
  - normal step execution remains concurrent in isolated worktrees
- Wave lifecycle alignment:
  - local executor keeps host-side eager worktree behavior
  - Docker executor defers workspace prep to execution and cleans up Docker worktrees on delete
- Explicit safety guard:
  - `fork(select: all)` fails fast in Docker mode to avoid unsafe shared-workspace branch concurrency

## Locked decisions

- Volume lifecycle: persistent per repo
- Volume identity: canonical repo URL, fallback to path hash
- Git model: shared clone + per-wave worktrees
- Lock scope: shared clone mutations only
- Credential mount/env behavior: unchanged in this phase
- Container cleanup for this phase: aggressive cleanup only for loopflow-managed containers

## Remaining work (next commits)

### High-priority follow-ups

1. Isolated per-branch Docker workspaces for `fork(select: all)` so safe parallel branch execution can be re-enabled.
2. Typed credential config and mount allowlist enforcement.
3. Durable container metadata persistence/recovery across daemon restarts.
4. Repo image build/rebuild pipeline (`_docker-gen`) integration.
5. Expanded daemon-level Docker e2e coverage.

### Known operational risks

- Helper container overhead during repeated git prep on busy systems.
- Persistent volume growth without pruning/retention policy.
- Prompt assembly still depends on host-side sync correctness.

## Validation status for this phase

- Added coverage for repo key normalization/fallback behavior.
- Added coverage for Docker workspace mount type (volume, not host bind).
- Added locking serialization tests for shared mutation paths.
- Added run-path progression checks for Docker execution flow.

This document is the current baseline for remote sandboxed agent work; superseded planning/review drafts were consolidated into this version.
