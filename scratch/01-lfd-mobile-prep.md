# lfd: Mobile Prep

Extend lfd to support mobile clients via HTTP API and WebSocket streaming.

---

## Problem

Mobile clients need to connect to lfd, but lfd's control plane is gRPC-only. Mobile apps (particularly iOS) prefer HTTP/WebSocket. The current HTTP layer is minimal (health checks only), and all streaming happens over gRPC.

Phase 2 of Concerto requires mobile clients to:
- See wave status in real-time
- Trigger non-interactive actions (run step, land PR)
- Receive live updates when local git operations happen

Without this, mobile is dead—it can't even list waves.

---

## Approach

Add a REST API and WebSocket endpoint to lfd's existing HTTP server. Reuse the existing `EventHub` and `OutputHub` broadcast channels—they're already there for gRPC streaming.

**Key insight:** The infrastructure exists. gRPC streaming works. We're adding HTTP transport, not rebuilding the event system.

**Auth model:** Trust localhost, require JWT for remote. This matches the existing gRPC auth pattern in `auth.rs`.

---

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Expose gRPC to mobile | gRPC on iOS is awkward (grpc-swift is heavy, no native support) | Poor mobile DX, larger binary size |
| HTTP polling only | Simpler, no WebSocket | Conductor persona needs instant updates, polling lag kills the experience |
| Relay events through loopflow.studio | Centralizes traffic, solves NAT | Adds latency, makes studio a SPOF, privacy concerns with event data |
| Server-Sent Events (SSE) | Simpler than WebSocket | One-directional, can't send ping/pong, worse iOS URLSession support |
| GraphQL subscriptions | Schema-driven, typed | Over-engineered for event streaming, adds complexity without benefit |

---

## Key decisions

### 1. WebSocket auth via query param

JWT passed as `?token=<jwt>` on WebSocket upgrade. Reason: WebSocket handshake doesn't support custom headers in browser/iOS URLSession. This is standard practice (Socket.io, Phoenix, etc.).

```
wss://100.x.x.x:2486/ws?token=<jwt>
```

### 2. No topic filtering for v0

All events go to all connected clients. The event volume is low (wave state changes, step completions). Adding topic subscriptions adds complexity without clear benefit at this scale.

If this becomes a problem later, add optional `?filter=wave.*` query param.

### 3. Reuse existing broadcast channels

`EventHub` and `OutputHub` already exist. Share them with the HTTP layer via `HttpState`. Don't create parallel event systems.

```rust
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,  // Add for action endpoints
    pub event_hub: EventHub,          // Add for WebSocket
    pub output_hub: OutputHub,        // Add for future output streaming
    pub auth: AuthContext,            // Add for JWT validation
    pub registration: Option<RegistrationClient>,
    pub started_at: OffsetDateTime,
}
```

### 4. Git hooks call localhost, not remote

Hooks always call `http://localhost:2486/hooks/git`. They run on the same machine as lfd. No auth needed. lfd translates these into events and broadcasts to all clients.

### 5. GitHub polling, not webhooks

Self-hosted lfd can't receive webhooks (no public URL). Poll GitHub API every 30 seconds for active waves. Use conditional requests (`If-None-Match`) to minimize API quota usage.

Managed hosting (future) will use webhooks. The event bus is source-agnostic—clients don't know or care where events originate.

### 6. Axum built-in WebSocket support

Axum has native WebSocket support via `axum::extract::ws`. No need to add `tokio-tungstenite` as a direct dependency. This keeps the dependency graph clean.

### 7. TLS for remote connections

Remote connections must use HTTPS/WSS. lfd already binds to `0.0.0.0:2486`. For TLS:
- Use `rustls` with self-signed cert generated on first run
- Store cert at `~/.lfd/server.{crt,key}`
- Tailscale MagicDNS provides free certs via ACME—document as recommended setup

### 8. Connection resilience via initial state dump

On WebSocket connect, immediately send current state snapshot before streaming deltas:

```json
{"type": "connected", "waves": [...], "timestamp": "..."}
```

This eliminates the need for "catch up" logic after reconnect—client just uses fresh state.

### 9. Heartbeat every 30 seconds

Server sends `{"type": "ping"}`, client responds with `{"type": "pong"}`. Detects dead connections faster than TCP keepalive. Axum WebSocket handles the low-level ping/pong frames automatically.

---

## Scope

### In scope

**HTTP API endpoints:**

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/waves` | List waves with current state |
| POST | `/api/waves` | Create wave |
| GET | `/api/waves/:id` | Get wave detail |
| PATCH | `/api/waves/:id` | Update wave config |
| DELETE | `/api/waves/:id` | Delete wave |
| POST | `/api/waves/:id/run` | Run wave (triggers step execution) |
| POST | `/api/waves/:id/stop` | Stop running wave |
| POST | `/api/waves/:id/land` | Land PR for wave |

**WebSocket endpoint:**

| Endpoint | Purpose |
|----------|---------|
| GET `/ws` | Upgrade to WebSocket for event stream |

**Git hooks:**

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/hooks/git` | Receive git hook notification |

**Auth:**
- Localhost (`127.0.0.1`, `::1`) → no auth required
- Remote IPs → require `Authorization: Bearer <JWT>` header (or `?token=` for WebSocket)

**Event types:**
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

**GitHub polling:**
- Poll every 30 seconds for waves with open PRs
- Track PR state (open, merged, closed)
- Track CI check status (pending, success, failure)
- Use conditional requests to minimize API calls

**Git hooks installation:**
- `post-commit`, `post-checkout`, `post-merge`, `post-rewrite`
- Installed via `lfd hooks install <repo>` command
- Hook script calls `curl http://localhost:2486/hooks/git`

### Out of scope

- GitHub webhooks (managed hosting only, future)
- APNS push notifications (separate roadmap item)
- Terminal streaming over HTTP (gRPC-only for now)
- UI changes (this is lfd backend only)
- OAuth flow (loopflow.studio handles this)
- JWT issuance (loopflow.studio handles this)
- Rate limiting (add if abuse occurs)

---

## Status (as of February 4, 2026)

### Implemented

- HTTP REST + WebSocket endpoints for wave CRUD, actions, and live event streaming.
- Shared `EventHub` broadcasting for both gRPC subscriptions and WebSocket clients.
- loopflow.studio registration + heartbeat, connection-token validation, and HTTP/gRPC auth checks.
- Git hooks now POST localhost HTTP events instead of using the UNIX socket.
- Machine identity + config/credentials loading wired into startup.

### Key choices confirmed in code

- WebSocket auth accepts `?token=` or Authorization headers; localhost bypasses auth.
- HTTP binds to `0.0.0.0:2486` and requires registration for non-loopback access.
- Event payloads are normalized to JSON with `type` + `data`.
- Registration failure allows local access while blocking remote access.

### Not included yet

- GitHub polling for PR/CI status.
- TLS certificate management for remote HTTPS/WSS.
- Output streaming over HTTP (reserved via `OutputHub` but not wired).

### Risks and bottlenecks

- Remote access depends on loopflow.studio registration + connection-token validation; mobile auth mismatch would block clients.
- HTTP event payloads mirror proto structures; proto changes need JSON mapping updates.
- `run_wave_handler`/`land_wave_handler` spawn blocking tasks; heavy load could saturate the blocking thread pool.

### Open questions

Tracked in `scratch/questions.md`.

---

## Implementation order

1. **Wire EventHub and executor into HttpState** — Add `event_hub`, `output_hub`, `executor`, and `auth` to `HttpState`. Two-line struct change, two-line wiring change in main.rs. Unlocks everything else.

2. **Add WebSocket endpoint** — `/ws` handler that:
   - Validates JWT for remote clients (skip for localhost)
   - Upgrades connection
   - Sends initial state snapshot
   - Subscribes to EventHub and streams events
   - Handles ping/pong
   - Test with `websocat ws://localhost:2486/ws`

3. **Add wave CRUD endpoints** — `/api/waves` routes that delegate to existing `RunStore` methods. The store layer already has `list_waves`, `get_wave`, `create_wave`, `update_wave`, `delete_wave`.

4. **Add wave action endpoints** — `/api/waves/:id/run`, `/stop`, `/land`. Delegate to `WaveExecutor` methods that already exist from gRPC implementation.

5. **Add localhost bypass to auth** — Before checking JWT, check if peer addr is `127.0.0.1` or `::1`. If so, skip auth.

6. **Add git hook endpoint** — `/hooks/git` that:
   - Accepts POST with JSON body `{"hook": "post-commit", "repo": "/path/to/repo"}`
   - Looks up wave by repo path
   - Broadcasts `wave.state_changed` event
   - No auth (localhost only, enforced by hook script)

7. **Add GitHub polling loop** — Background task spawned at startup that:
   - Every 30 seconds, queries store for waves with open PRs
   - For each, calls GitHub API to check PR state and CI status
   - Broadcasts events for state changes
   - Uses `If-None-Match` headers for conditional requests

8. **Add hook installation command** — `lfd hooks install <repo>` that writes hook scripts to `.git/hooks/`.

---

## Done when

```bash
# WebSocket streams events (with initial state)
websocat ws://localhost:2486/ws
# Should immediately receive: {"type": "connected", "waves": [...]}

# Wave CRUD works
curl http://localhost:2486/api/waves
curl -X POST http://localhost:2486/api/waves -d '{"name": "test", ...}'
curl http://localhost:2486/api/waves/123
curl -X PATCH http://localhost:2486/api/waves/123 -d '{"paused": true}'
curl -X DELETE http://localhost:2486/api/waves/123

# Actions trigger execution
curl -X POST http://localhost:2486/api/waves/123/run
curl -X POST http://localhost:2486/api/waves/123/stop
curl -X POST http://localhost:2486/api/waves/123/land

# Git operations broadcast events
git commit -m "test"  # triggers wave.state_changed event over WebSocket

# Remote access requires JWT
curl -H "Authorization: Bearer <jwt>" https://100.x.x.x:2486/api/waves

# Hook installation works
lfd hooks install .
cat .git/hooks/post-commit  # Should show curl to localhost
```

**Verification:** iOS simulator connects to lfd, sees wave list, receives live updates when git operations happen on laptop.

---

## Success looks like

Six months from now:
- Conductors check wave status from their phones while commuting
- "Land PR" from the couch works flawlessly
- Nobody thinks about the WebSocket connection—it just works
- The API is stable enough that we haven't changed it since v0
- Response times are under 100ms for all endpoints
- WebSocket reconnection is seamless (initial state dump makes it trivial)

## Failure looks like

Six months from now:
- WebSocket connections drop constantly, users give up on mobile
- We built a REST API that doesn't match what the iOS app actually needs
- TLS setup is so painful that users only use localhost
- GitHub polling hammers the API and runs out of quota
- Event ordering issues cause UI glitches
- We're on v3 of the API because v0 was wrong

**Mitigations built into this design:**
- Initial state dump eliminates reconnection complexity
- API matches gRPC 1:1 to avoid mismatched abstractions
- Tailscale recommended for TLS (free certs, easy setup)
- Conditional requests + 30s polling keeps GitHub API usage minimal
- Events are idempotent—duplicate delivery is fine
