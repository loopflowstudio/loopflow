# Remote Roadmap

Connect Concerto to a remote lfd. Same HTTP+WS protocol, different host.

## North Star

lfd runs on a remote Linux machine (containerized). Concerto connects from your Mac (or phone). Waves run remotely with full agent execution. You open Cursor via Remote SSH when you need to edit files. One-click per wave.

## Phases

| # | Phase | What it unlocks | Pre-work | Status |
|---|-------|----------------|----------|--------|
| 01 | Sandboxed Agents | Agents in Docker containers, controlled credentials | None | Shipped |
| 02 | Compose Stack | Full stack in Docker (lfd + postgres), test locally | 01 | Shipped |
| 03 | Pre-shared Token Auth | lfd accepts remote connections | None | Shipped |
| 04 | EC2 Infrastructure | A box to deploy on (Docker + compose) | 02 | Shipped |
| 05 | Concerto Remote Connection | Wave CRUD, events, logs over WAN | 03, 04 | Next |
| 06 | Remote File Access | One-click "Open in Cursor" per wave | 05 |
| 07 | Studio Auth | Real JWT auth via auth.loopflow.studio (server built, client wiring remaining) | 05 |
| 08 | API Expansion | File browsing, step/flow/direction typeahead | 05 |
| 09 | Hosted SaaS | loopflow.studio runs lfd for you | 07, 08 |

Phases 06, 07, and 08 can run in parallel after 05.

Phase 05 implementation is intentionally capped to a single PR with a practical size guardrail. Any non-critical scope cut from 05 rolls forward into Phase 06/08 docs instead of expanding the 05 PR.

## Update after Phase 01D hardening

Phase 01D shipped and changed what we now treat as non-negotiable for remote work:

- Fork semantics are the same in CLI and daemon: run all branches, then synthesize.
- Scheduler slot release and orphan-fork cleanup must be restart-safe.
- Agent timeout is explicit operator config (`executor.agent_timeout`), not hidden watchdog behavior.
- Fork execution is unsupported in the Docker executor — no worktree-based parallelism in containers. Remote users on Docker are limited to non-fork flows. This is an accepted gap, not a blocker.
- Track dedicated follow-up in `wave/remote/01e-docker-fork-parity.md`.

### Impact on the next phase (05)

Phase 05 now needs to include two correctness checks that were previously implicit:

1. Timeout/fail-fast errors from daemon execution need clear surfacing in Concerto (not generic request failures).
2. Chat events use SSE (`GET /waves/:id/chat/events`), not WebSocket. Phase 05 must verify SSE works through the TLS proxy (Caddy) alongside the existing WebSocket path for run events.

### Open questions for 05/06

- Do we expose `executor.agent_timeout` in Concerto connection settings, or keep it daemon-config only in v1?
- Should remote capability warnings live in wave detail only, or also in wave edit/config surfaces?

### What might change next

If Docker build-context cost is painful on large repos during Phase 05 dogfooding, we may need to pull that optimization earlier than currently planned.

## Architecture

```
Local (native, default):
  Concerto ──HTTP/WS──▶ lfd (127.0.0.1:2486, native process)
  lfd ──Docker API──▶ agent containers (sandboxed)   [Phase 01]

Containerized local (docker compose up):
  Concerto ──HTTP/WS──▶ lfd (127.0.0.1:2486, Docker)   [Phase 02]
  lfd ──Docker API──▶ agent containers (siblings via socket)
  postgres (container, auto-migrated on startup)

Remote (Phase 04+):
  Concerto ──HTTPS/WSS──▶ lfd (ec2-host:2486, Docker + Caddy TLS)
  Cursor ──Remote SSH──▶ ec2-host:/path/to/worktree
  Auth: static token (Phase 03) or JWT (Phase 07)
```

lfd is already the remote server. Concerto is already a thin client. The protocol doesn't change — only the host and transport.

## Design Decisions

**Sandbox first.** Phase 01 gives you sandboxed agents locally — security without changing how lfd runs. Phase 02 packages the whole stack into Docker Compose for deployment.

**File access:** Use each editor's native remote support (Cursor Remote SSH, JetBrains Gateway, Zed Remote). Loopflow owns the orchestration (which host, which worktree) — editors own the transport.

**Auth sequence:** Pre-shared static token for dev testing (Phase 03, shipped). `AuthProvider` enum (`Local`, `Static`, `Studio`) is extensible — Phase 07 adds JWT validation to the existing `Studio` variant.

**No filesystem mounts.** lfd serves file data through API endpoints. Editors connect via their own remote protocols. No SSHFS, no Mutagen, no FUSE.

## Dependencies from rust/

- [Service installation](../rust/03-service.md) — systemd on Linux, optional if using Docker

## What's not here

- Mobile UI — see [mobile/](../mobile/)
- Wave area summaries — separate project
- UX polish (optimistic updates, etc.) — see [ux/](../ux/)
