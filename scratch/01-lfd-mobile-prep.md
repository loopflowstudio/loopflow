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
    pub event_hub: EventHub,        // Add this
    pub output_hub: OutputHub,      // Add this
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

---

## Scope

### In scope

**HTTP API endpoints:**

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/waves` | List waves with current state |
| POST | `/waves` | Create wave |
| GET | `/waves/:id` | Get wave detail |
| PATCH | `/waves/:id` | Update wave config |
| DELETE | `/waves/:id` | Delete wave |
| POST | `/waves/:id/run` | Run wave (triggers step execution) |
| POST | `/waves/:id/stop` | Stop running wave |
| POST | `/waves/:id/land` | Land PR for wave |

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
{"type": "wave.created", "wave_id": "...", "name": "..."}
{"type": "wave.state_changed", "wave_id": "...", "state": "running"}
{"type": "wave.deleted", "wave_id": "..."}
{"type": "step.started", "wave_id": "...", "step": "implement"}
{"type": "step.completed", "wave_id": "...", "step": "implement", "success": true}
{"type": "pr.status_changed", "wave_id": "...", "pr_url": "...", "status": "merged"}
{"type": "ci.completed", "wave_id": "...", "passed": true}
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

---

## Implementation order

1. **Share EventHub with HTTP layer** — Wire `event_hub` and `output_hub` into `HttpState`. Minimal change, unlocks everything else.

2. **Add WebSocket endpoint** — `/ws` handler that upgrades and streams from `EventHub`. Test with `websocat`.

3. **Add wave CRUD endpoints** — `/waves` routes that delegate to existing `RunStore` methods. The store layer already has everything.

4. **Add wave action endpoints** — `/waves/:id/run`, `/stop`, `/land`. Call existing executor methods.

5. **Add JWT auth middleware** — Extract and validate JWT for non-localhost requests. Reuse `ConnectionValidator` pattern from gRPC.

6. **Add git hook endpoint** — `/hooks/git` that translates hook events into `EventHub` broadcasts.

7. **Add GitHub polling loop** — Background task that polls GitHub API and feeds results into `EventHub`.

8. **Add hook installation command** — `lfd hooks install <repo>` that writes hook scripts to `.git/hooks/`.

---

## Done when

```bash
# WebSocket streams events
websocat ws://localhost:2486/ws

# Wave CRUD works
curl http://localhost:2486/waves
curl -X POST http://localhost:2486/waves -d '{"name": "test", ...}'

# Actions trigger execution
curl -X POST http://localhost:2486/waves/123/run
curl -X POST http://localhost:2486/waves/123/land

# Git operations broadcast events
git commit -m "test"  # triggers wave.state_changed event

# Remote access requires JWT
curl -H "Authorization: Bearer <jwt>" http://100.x.x.x:2486/waves
```

Verification: iOS simulator connects to lfd, sees wave list, receives live updates when git operations happen on laptop.
