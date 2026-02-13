# 01C: Sandboxed Agent Hardening & Validation

Harden Docker executor for production use: fork isolation, credential safety, image pipeline, and CI coverage.

## What exists after this

Docker executor handles concurrent forked branches safely, mounts credentials through a typed allowlist (read-only), rebuilds images on trigger conditions, and has CI coverage proving it works.

## Work items

### Fork isolation

Isolated per-branch Docker workspaces so `fork(select: all)` can safely run parallel branch execution. Each forked branch gets its own worktree inside the repo volume, and concurrent container spawns don't interfere.

### Credentials

Typed mount config with hard allowlist and read-only semantics. Replace any raw `host:container` bind strings with a structured credential config. Only paths on the allowlist can be mounted. All credential mounts are forced read-only.

### Image pipeline

- Explicit rebuild triggers: `.lf/.docker-stale`, `.lf/Dockerfile` hash, `.lf/env-setup.sh` hash, base image ref change, image missing
- `_docker-gen` writes `.lf/Dockerfile` to repo when missing
- Per-image build lock + waiters for concurrent waves sharing the same image

### Tests

- PR CI: Docker smoke coverage (basic spawn, log, stop, cleanup)
- Nightly: full Docker e2e coverage (run, logs, cancel, cleanup, concurrent waves)

## Dependencies

- Stage 01A (shipped): executor trait, Docker backend, log streaming, cleanup
- Stage 01B (shipped): repo volumes, git worktrees, workspace hygiene, container durability

## Success criteria

- `fork(select: all)` runs parallel agents in isolated workspaces without interference
- Credential mounts are typed, allowlisted, and read-only — no raw bind strings
- Image rebuilds happen automatically when Dockerfile or env-setup changes
- Concurrent waves waiting on the same image build don't duplicate work
- Docker smoke tests run in PR CI; full e2e runs nightly
