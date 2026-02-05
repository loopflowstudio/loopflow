# lfd: Mobile Prep

HTTP API and WebSocket streaming for mobile clients.

---

## What's built

lfd exposes a REST API and WebSocket endpoint on `0.0.0.0:2486`. gRPC has been removed entirely — HTTP is the only transport.

**HTTP endpoints:**

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/waves` | List waves with current state |
| POST | `/api/waves` | Create wave |
| GET | `/api/waves/:id` | Get wave detail |
| PATCH | `/api/waves/:id` | Update wave config |
| DELETE | `/api/waves/:id` | Delete wave |
| POST | `/api/waves/:id/run` | Run wave |
| POST | `/api/waves/:id/stop` | Stop running wave |
| POST | `/api/waves/:id/land` | Land PR for wave |
| POST | `/hooks/git` | Receive git hook notification |
| GET | `/health`, `/status`, `/metrics` | Health and diagnostics |

**WebSocket:** `/ws` with initial state snapshot, live event streaming, 30-second ping/pong heartbeat.

**Auth:** Localhost bypasses auth. Remote requires loopflow.studio registration + connection token validation (cached 60s).

**Registration:** lfd registers with loopflow.studio on startup, heartbeats every 30s, deregisters on shutdown. Registration failure allows local access, blocks remote.

**Git hooks:** Python hook installer writes scripts that POST to `http://127.0.0.1:2486/hooks/git` in background.

**EventHub:** Single broadcast system shared across HTTP handlers and loop tasks. All WebSocket clients receive all events.

---

## Architecture

```
                    ┌─────────────────┐
                    │ loopflow.studio │
                    │   (auth/token)  │
                    └────────┬────────┘
                             │ register/validate
                             ▼
┌──────────┐     HTTP     ┌─────┐     EventHub     ┌────────────┐
│  Mobile  │─────────────►│ lfd │◄────────────────►│ Loop tasks │
└──────────┘     WS       └─────┘                  └────────────┘
                             ▲
                             │ POST /hooks/git
                    ┌────────┴────────┐
                    │   Git hooks     │
                    │ (post-commit..) │
                    └─────────────────┘
```

`HttpState` holds `store`, `scheduler`, `executor`, `event_hub`, `auth`, `registration`. Handlers delegate to store operations and broadcast events. Run/land handlers spawn blocking tasks for execution.

---

## Key decisions

| Decision | Rationale |
|----------|-----------|
| HTTP over gRPC for mobile | grpc-swift is heavy; iOS URLSession works natively with HTTP/WebSocket |
| WebSocket over SSE | Bidirectional for ping/pong; better iOS support |
| Connection token auth (not raw JWT) | Token scoped to lfd connection; loopflow.studio validates |
| Initial state snapshot on connect | Eliminates reconnection catch-up complexity |
| Single transport (HTTP only) | gRPC wasn't used by any client; single transport simplifies deployment |
| No topic filtering for v0 | Event volume is low; add `?filter=wave.*` later if needed |
| Git hooks call localhost | Same machine as lfd, no auth needed |
| GitHub polling, not webhooks | Self-hosted lfd can't receive webhooks; poll every 30s with conditional requests |

---

## What remains

**GitHub polling for PR/CI status.** Waves with open PRs need PR state (open/merged/closed) and CI status (pending/success/failure) tracked. Poll GitHub API every 30 seconds using conditional requests (`If-None-Match`).

**TLS for remote connections.** Remote access requires HTTPS/WSS. Options: self-signed cert at `~/.lfd/server.{crt,key}` or Tailscale MagicDNS (recommended).

**Output streaming over HTTP.** `OutputHub` is reserved in `HttpState` but not wired to any endpoint.

**`lfd hooks install` CLI command.** Hooks are installable via Python, but no Rust CLI command yet.

---

## Event types

```json
{"type": "connected", "waves": [...], "timestamp": "..."}
{"type": "ping"}
{"type": "wave.created", "wave_id": "...", "name": "..."}
{"type": "wave.updated", "wave_id": "...", "wave": {...}}
{"type": "wave.deleted", "wave_id": "..."}
{"type": "wave.state_changed", "wave_id": "...", "state": "running"}
{"type": "step.started", "wave_id": "...", "step": "implement"}
{"type": "step.completed", "wave_id": "...", "step": "implement", "success": true}
{"type": "pr.opened", "wave_id": "...", "pr_url": "...", "pr_number": 123}
{"type": "pr.status_changed", "wave_id": "...", "pr_url": "...", "status": "merged"}
{"type": "ci.status_changed", "wave_id": "...", "status": "success"}
```

---

## Risks

| Risk | Mitigation |
|------|------------|
| Remote access blocked if loopflow.studio is down | Localhost always works; registration failure is graceful |
| Token validation latency | 60-second cache reduces round-trips |
| Blocking thread pool saturation from run/land | Pool sized to CPU count; acceptable for v0 |
| Event ordering for reconnecting clients | Initial snapshot provides full state; events are idempotent |
| HTTP event payloads mirror proto structures | Proto changes need JSON mapping updates |

---

## Verification

```bash
websocat ws://localhost:2486/ws          # WebSocket with initial state
curl http://localhost:2486/api/waves     # Wave list
curl http://localhost:2486/health        # Health check
cargo test && cargo clippy -- -D warnings && uv run pytest tests/
```
