# 05: Concerto Remote Connection

Wave CRUD, event streaming, and log access over WAN. Concerto connects to a remote lfd the same way it connects to a local one — same HTTP/WS surface, different host.

Implementation is intentionally capped to a single PR with a practical size guardrail. Any non-critical scope cut rolls forward into 06/08 docs instead of expanding this PR.

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

## Design decisions

**Sandbox first.** Phase 01 gives you sandboxed agents locally — security without changing how lfd runs. Phase 02 packages the whole stack into Docker Compose for deployment.

**File access:** Use each editor's native remote support (Cursor Remote SSH, JetBrains Gateway, Zed Remote). Loopflow owns the orchestration (which host, which worktree) — editors own the transport.

**Auth sequence:** Pre-shared static token for dev testing (Phase 03, shipped). `AuthProvider` enum (`Local`, `Static`, `Studio`) is extensible — Phase 07 adds JWT validation to the existing `Studio` variant.

**No filesystem mounts.** lfd serves file data through API endpoints. Editors connect via their own remote protocols. No SSHFS, no Mutagen, no FUSE.

## Correctness checks (from Phase 01D)

1. Timeout/fail-fast errors from daemon execution need clear surfacing in Concerto (not generic request failures).
2. Chat events use SSE (`GET /waves/:id/chat/events`), not WebSocket. Must verify SSE works through the TLS proxy (Caddy) alongside the existing WebSocket path for run events.

## Dependencies

- [Service installation](../rust/03-service.md) — systemd on Linux, optional if using Docker

## Done when

- Concerto connects to a remote lfd over HTTPS/WSS
- Wave CRUD, event streaming, and log access work over WAN
- SSE chat events and WebSocket run events both succeed through TLS proxy
