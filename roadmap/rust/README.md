# Rust Roadmap

Source of truth for the Rust control plane and path to hosted teams.

## North Star

A Rust-based control plane driven locally or remotely (desktop + mobile), with strict isolation between control and execution.

**Design priorities:**
- Rust-only distribution (no Python runtime, binaries via PyPI like ruff/uv)
- HTTP-only protocol (axum, WebSocket for events, no gRPC)
- Security by default (auth required for remote access)
- Claude Pro/Max support (no API keys required)
- Progressive complexity (local is simple, hosted adds features)

## Phases

| Phase | Focus | lfd Runs | Agents Run | Claude Auth | User Auth |
|-------|-------|----------|------------|-------------|-----------|
| 1 | Rust release | Local | Local process | ~/.claude | None (localhost) |
| 2 | Self-hosted | Self-hosted | Local/Container/K8s | ~/.claude (mounted) | JWT via loopflow.studio |
| 3 | Hosted teams | Our cloud | K8s Jobs | Device flow | JWT via loopflow.studio |

### Phase 1: Rust Release ✅

Ship `lf` and `lfd` as Rust binaries. No Python runtime required.

| Task | Status |
|------|--------|
| Prompt parity (Rust matches Python output) | ✅ |
| Ops parity (commit, land, pr, next, etc.) | ✅ |
| Binary distribution (install.sh, crates.io, PyPI) | ✅ |
| Remove PyO3/Python bindings | ✅ |
| Merge lfd into lf crate | ✅ |
| Port `lf ops cp` and `lf ops doctor` | ✅ |
| CI: release workflow builds + publishes | ✅ |
| Event emission (EventHub → WebSocket) | ✅ |
| Add builtin steps: `add-prompt`, `setup` | 🔜 |
| Service installation (launchd/systemd) | 🔜 |

**Remaining Phase 1 docs:**

| Doc | Scope |
|-----|-------|
| [02b-summarize](02b-summarize.md) | Wave area summaries for LLM context |
| [03-service](03-service.md) | launchd/systemd service installation |

### Phase 2: Self-Hosted with Auth

Enable remote access with authentication. Containers optional but supported.

| Doc | Scope |
|-----|-------|
| [04-auth](04-auth.md) | loopflow.studio auth, JWT validation, axum middleware |
| [05-infrastructure](05-infrastructure.md) | Executor abstraction, containers, K8s, deployment |

### Phase 3: Hosted Teams

Full SaaS control plane. Same infrastructure self-hosters use, but we run it.

| Doc | Scope |
|-----|-------|
| [06-hosted](06-hosted.md) | Control plane, multi-tenancy, web terminal, billing |

## Current State

**lfd architecture (HTTP-only):**
- axum HTTP server with REST endpoints for wave/stimulus/agent CRUD
- WebSocket endpoint for real-time events (EventHub) and output streaming (OutputHub)
- SQLite storage (rusqlite, synchronous)
- Background triggers: loop ticker (5s), watch poller (30s), cron poller, recovery
- Scheduler with semaphore-based slot management

**Distribution (all working):**
- `curl -fsSL .../install.sh | sh` — all platforms
- `cargo install loopflow` — from crates.io
- `uv tool install loopflow` — Python CLI (lfq) only

## Principles

- **Rust-only:** No Python runtime. Binaries distributed via PyPI (like ruff, uv).
- **HTTP-only:** axum for REST, WebSocket for events. No gRPC.
- **Self-extending:** Missing features become builtin steps, not code.
- **Security by default:** Auth required for any remote access.

## Open Questions

| Question | Options | Decision |
|----------|---------|----------|
| Homebrew tap | Create tap repo | Nice to have, not blocking |
| Windows support | TCP instead of Unix socket | Not yet |
