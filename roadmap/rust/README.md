# Rust Roadmap

Pre-work items for the Rust control plane. Most remote/hosted work has moved to [remote/](../remote/).

## Phase 1: Rust Release (complete)

Ship `lf` and `lfd` as Rust binaries. No Python runtime required.

| Task | Status |
|------|--------|
| Prompt parity (Rust matches Python output) | Done |
| Ops parity (commit, land, pr, next, etc.) | Done |
| Binary distribution (install.sh, crates.io, PyPI) | Done |
| Remove PyO3/Python bindings | Done |
| Merge lfd into lf crate | Done |
| Port `lf ops cp` and `lf ops doctor` | Done |
| CI: release workflow builds + publishes | Done |
| Event emission (EventHub -> WebSocket) | Done |

## Pre-work

| Doc | What | Enables |
|-----|------|---------|
| [03-service](03-service.md) | systemd/launchd service installation | Optional if using Docker |

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
- **Security by default:** Auth required for any remote access.
