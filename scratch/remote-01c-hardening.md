# 01C: Sandboxed Agent Hardening & Validation

Harden Docker executor for production use: fork isolation, credential safety, image pipeline, and CI coverage.

## Problem

The Docker executor ships agents in containers but has three production gaps:

1. **Fork isolation**: `fork(select: all)` is hard-rejected for Docker. Waves that fan out across directions (e.g., `roadmap-reduce` running reduce×3) can't use containers. This blocks the primary loopflow workflow.

2. **Credential exposure surface**: Credentials mount via raw `host:container` strings in YAML. Any host path can be mounted. There's no validation that mounted paths are known credential locations, no protection against typos mounting `/etc/passwd`.

3. **Image staleness**: The executor uses a static `loopflow/agent:latest` image. When `.lf/Dockerfile` or `env-setup.sh` changes, the running image doesn't update. Users must manually rebuild. Concurrent waves can trigger duplicate builds.

4. **No Docker test coverage**: Zero Docker tests in CI. The executor is ~900 lines of untested production code.

## Approach

Four workstreams, ordered by dependency. Fork isolation unblocks the core workflow. Credentials harden the security boundary. Image pipeline removes manual rebuild friction. Tests prove it all works.

### 1. Fork isolation in Docker

Remove the `executor_type == Docker` early-return in `run_fork()`. Make Docker forks work by preparing isolated worktrees inside the same repo volume.

**How it works:**

Each fork branch gets its own worktree in the Docker volume, mirroring the local-mode pattern:

```
Docker volume: lfd-repo-{hash}
  /workspace/repos/{repo-key}/
    main/                          # shared clone
    worktrees/
      wave-slug/                   # main wave worktree
      wave-slug-fork-0/            # fork branch 0
      wave-slug-fork-1/            # fork branch 1
      wave-slug-fork-2/            # fork branch 2
```

The existing `prepare_workspace()` already creates worktrees inside volumes. Fork isolation reuses this path — each fork branch calls `prepare_workspace()` with a fork-specific workspace derived from `fork_worktree_path()`.

**Key changes:**

- `run_fork()`: remove Docker rejection. Build a `DockerWorkspace` per fork branch (volume + fork-specific worktree path).
- `DockerExecutor::run()`: accept the fork workspace's container path (already parametric on `cwd`).
- `prepare_workspace()`: no changes needed — it already takes an arbitrary `DockerWorkspace`.
- Sync-back: each fork container syncs its worktree back to its own host fork worktree on exit.
- Mutation locks: fork worktrees share the same repo key lock for clone mutations, but run hygiene independently.

**Concurrency model:**

Fork branches run in parallel containers. Each container writes to its own worktree. The shared clone is locked only during `ensure_shared_clone` and `fetch`, not during agent execution. This is identical to how local-mode forks work (parallel processes, separate worktrees).

### 2. Typed credential allowlist

Replace raw `host:container` mount strings with a typed credential config that validates against a hard-coded allowlist.

**Allowlist:**

```rust
const CREDENTIAL_ALLOWLIST: &[&str] = &[
    ".claude",           // Claude Code (Max/Pro)
    ".codex",            // Codex CLI (ChatGPT Plus/Pro)
    ".config/gemini",    // Gemini CLI
    ".gitconfig",        // git config
    ".ssh",              // SSH keys (git push)
    ".gnupg",            // GPG keys (signed commits)
];
```

All paths are relative to `$HOME`. Absolute paths and paths outside `$HOME` are rejected at config parse time.

**New config format:**

```yaml
executor:
  credentials:
    env:
      - ANTHROPIC_API_KEY
      - GH_TOKEN
    mounts:
      - claude     # → ~/.claude:/home/agent/.claude (ro)
      - codex      # → ~/.codex:/home/agent/.codex (ro)
      - gitconfig  # → ~/.gitconfig:/home/agent/.gitconfig (ro)
      - ssh        # → ~/.ssh:/home/agent/.ssh (ro)
```

Credential names map to allowlisted paths. Shorthand names (without dots/slashes) are resolved against the allowlist. The old `host:container` format is rejected at parse time — no backwards compatibility shim.

**Implementation:**

- `CredentialMount` enum: `Named(String)` for allowlist names.
- `resolve_credential_mount()`: maps names to `(host_path, container_path)` pairs. Returns error for unknown names.
- Container path mirrors host path structure under `/home/agent/`.
- All mounts forced read-only (already the case).
- `DockerCredentialMount::from_spec()` replaced by `CredentialMount::resolve()`.

### 3. Image pipeline

Build images on-demand from `.lf/Dockerfile` (or a generated default) with content-addressed rebuild triggers.

**Trigger conditions:**

| Condition | Detection |
|-----------|-----------|
| Image missing | `docker image inspect` returns 404 |
| `.lf/Dockerfile` changed | SHA-256 of file content vs stored hash |
| `.lf/env-setup.sh` changed | SHA-256 of file content vs stored hash |
| `.lf/.docker-stale` exists | Sentinel file (user/tooling signal) |
| Base image ref changed | Parse `FROM` line, compare to stored ref |

**Image identity:**

Images are tagged per-repo: `lfd-agent-{repo-key}:latest`. The repo key is the same deterministic key used for volumes. This means each repo can have its own customized image without collision.

**Build flow:**

1. Before spawning a container, compute trigger fingerprint: `sha256(Dockerfile content + env-setup content + base_image_ref)`.
2. Compare against the tag's stored label `io.loopflow.build-fingerprint`.
3. If mismatch or image missing: build.
4. If `.lf/.docker-stale` exists: build, then delete the sentinel.
5. If `.lf/Dockerfile` doesn't exist: generate a default from `docker/agent/Dockerfile` template, write to `.lf/Dockerfile`, then build.

**Concurrent build coordination:**

```rust
struct ImageBuildLocks {
    inner: Arc<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
}
```

First wave to need a build acquires the lock and builds. Subsequent waves for the same image `await` the `Notify`. After build completes, all waiters proceed. No duplicate builds.

**Generated Dockerfile (`_docker_gen`):**

When `.lf/Dockerfile` is missing, generate one from the base agent image:

```dockerfile
FROM loopflow/agent:latest

# Project-specific setup
COPY .lf/env-setup.sh /tmp/env-setup.sh
RUN if [ -f /tmp/env-setup.sh ]; then sh /tmp/env-setup.sh; fi

WORKDIR /workspace
```

The generated file is committed to the repo so it's visible and editable. Users customize from there.

### 4. CI Docker coverage

**PR CI (smoke):**

Add a `docker-smoke` job to `.github/workflows/ci.yml`. Runs on `ubuntu-latest` (which has Docker). Tests:

- `DockerExecutor::new()` connects to Docker daemon
- `ensure_volume()` creates and removes a volume
- `run_helper_container()` executes a command and captures output
- `build_mounts_for()` produces correct mount specs
- Credential allowlist rejects unknown paths

These are Rust integration tests gated behind `#[cfg(feature = "docker-integration")]` or a `#[ignore]` attribute that CI explicitly runs via `cargo test -- --ignored`.

**Nightly CI (full e2e):**

Add a `docker-e2e` job to a nightly workflow. Tests:

- Spawn agent container, verify log streaming, wait for exit
- Cancel a running container, verify cleanup
- Two concurrent waves on the same repo volume, verify isolation
- Fork with 3 branches, verify all complete independently
- Image rebuild after Dockerfile change

These run as shell scripts in `tests/e2e/test_docker_*.sh`, similar to existing e2e tests.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-fork Docker volume (instead of per-fork worktree) | Stronger isolation, but requires cloning the repo N times per fork | Too slow. Git worktrees give filesystem isolation without duplicating objects. |
| Credential allowlist as config (instead of hard-coded) | More flexible | Flexibility is the enemy here. A configurable allowlist lets users mount anything. Hard-coded means we audit every path. |
| Content-addressed image tags (sha-based) instead of `:latest` | Immutable, cacheable | Over-engineering for single-machine daemon. The fingerprint label on `:latest` gives rebuild detection without tag proliferation. |
| Docker BuildKit cache mounts for image builds | Faster rebuilds | Adds BuildKit dependency. Standard `docker build` is fine for the infrequent rebuilds we expect. |
| Backwards-compatible credential mount format | Smooth migration | Raw `host:container` strings are the vulnerability. Keeping them "for compatibility" defeats the purpose. Break cleanly. |

## Key decisions

1. **Fork worktrees inside volumes, not separate volumes.** Follows the roadmap principle "one Docker volume per repo" (01A architecture). Worktrees share git objects, so 3 fork branches add ~zero storage overhead.

2. **Hard-coded credential allowlist, not configurable.** The security boundary is the point. Every mountable path is audited in code review. Users who need custom mounts can modify the allowlist and rebuild — that's intentional friction.

3. **Break the old credential mount format.** No `host:container` strings. Named mounts only. This is an internal config file, not a published API. The migration is: edit `~/.lf/lfd.yaml`, replace paths with names. One-time, trivial.

4. **Per-repo image tags, not global.** Repos have different dependencies. A Python ML repo and a TypeScript web app shouldn't share an agent image. Per-repo tags via `lfd-agent-{repo-key}:latest` are the natural unit.

5. **Smoke tests in PR CI, full e2e nightly.** Docker tests are slow. PR CI gets lightweight validation (seconds). Nightly gets the full concurrent-wave, fork-isolation, image-rebuild suite. This follows the existing CI pattern (e2e smoke in PR, heavier tests elsewhere).

## Scope

**In scope:**
- Docker fork isolation (remove rejection, wire up fork worktrees in volumes)
- Typed credential allowlist (replace raw mount strings)
- Image build pipeline (trigger detection, per-image locks, `_docker_gen`)
- PR smoke tests for Docker executor
- Nightly e2e tests for Docker workflows

**Out of scope:**
- Network policy/firewall rules for containers
- Per-wave credential scoping (all waves get same credentials)
- Multi-arch image builds
- Remote Docker daemon support (assumes local socket)
- Custom base image selection per wave
- Volume garbage collection

## Done when

```bash
# Fork isolation works
lf flow roadmap-reduce  # with executor.type: docker — runs 3 fork branches in parallel containers

# Credential allowlist enforced
# Config with raw host:container paths → parse error
# Config with `mounts: [claude, ssh]` → mounts ~/.claude and ~/.ssh read-only

# Image pipeline
# Delete agent image, run wave → image auto-built from .lf/Dockerfile
# Modify .lf/Dockerfile, run wave → image rebuilt
# Two waves trigger simultaneously → one build, both wait

# Tests pass
cargo test --all
cargo test -- --ignored                 # Docker smoke tests (requires Docker daemon)
./tests/e2e/test_docker_smoke.sh       # Docker e2e smoke
```
