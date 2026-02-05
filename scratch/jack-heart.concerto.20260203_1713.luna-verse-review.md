# lfd Mobile Prep: Review

Branch: `jack-heart.concerto.20260203_1713.luna-verse`

---

## What was implemented

HTTP API and WebSocket streaming for lfd, enabling mobile clients to connect without gRPC.

**HTTP endpoints:**
- Wave CRUD: `GET/POST /api/waves`, `GET/PATCH/DELETE /api/waves/:id`
- Wave actions: `POST /api/waves/:id/run`, `/stop`, `/land`
- Git hooks: `POST /hooks/git`
- Status: `/health`, `/status`, `/metrics`

**WebSocket:**
- `/ws` endpoint with initial state snapshot + live event streaming
- 30-second ping/pong heartbeat
- Auth via `?token=` query param or `Authorization` header

**Auth:**
- Localhost requests bypass auth
- Remote requests require loopflow.studio registration + connection token validation
- Token caching (60s) to reduce validation round-trips

**Registration:**
- lfd registers with loopflow.studio on startup
- 30-second heartbeat loop
- Deregistration on shutdown

**Git hooks:**
- Updated Python hook installer to POST to HTTP endpoint instead of UNIX socket
- Hooks run curl to `http://127.0.0.1:2486/hooks/git` in background

---

## Key choices

| Decision | Rationale |
|----------|-----------|
| HTTP over gRPC for mobile | grpc-swift is heavy; iOS URLSession works natively with HTTP/WebSocket |
| WebSocket over SSE | Bidirectional for ping/pong; better iOS support |
| Connection token auth (not raw JWT) | Token scoped to lfd connection; loopflow.studio validates |
| Initial state snapshot on connect | Eliminates reconnection catch-up complexity |
| Removed gRPC server entirely | Single transport simplifies deployment; gRPC wasn't used by any client |
| EventHub shared across HTTP/loops | Single event system; no duplication |

---

## How it fits together

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

- `HttpState` holds `store`, `scheduler`, `executor`, `event_hub`, `auth`, `registration`
- Handlers delegate to store operations and broadcast events
- Run/land handlers spawn blocking tasks for execution
- WebSocket subscribers receive all events from `EventHub`

---

## Risks and bottlenecks

| Risk | Mitigation |
|------|------------|
| Remote access blocked if loopflow.studio is down | Localhost always works; registration failure is graceful |
| Token validation latency | 60-second cache reduces round-trips |
| Blocking thread pool saturation | Run/land spawn blocking tasks; pool sized to CPU count |
| Event ordering for reconnecting clients | Initial snapshot provides full state; events are idempotent |

---

## What's not included

**Explicitly deferred:**
- GitHub polling for PR/CI status (design doc mentions; not implemented)
- TLS certificate management (recommends Tailscale MagicDNS)
- Output streaming over HTTP (OutputHub reserved but not wired)
- `lfd hooks install` CLI command (hooks installable via Python)

**Out of scope:**
- iOS client implementation (separate roadmap item)
- Push notifications (separate roadmap item)
- Rate limiting (add if needed)

---

## Test coverage

| Suite | Status |
|-------|--------|
| Rust tests | 5 passed, 1 ignored (PTY) |
| Python tests | 679 passed, 1 skipped |
| Clippy | Clean (warnings = errors) |
| Formatting | Clean |

---

## Files changed

| Category | Files |
|----------|-------|
| New HTTP layer | `http.rs` (945 lines) |
| New auth/registration | `auth.rs`, `registration.rs`, `credentials.rs`, `config.rs`, `machine_id.rs` |
| New types | `types/{mod,agent,event,stimulus,wave}.rs` |
| New event broadcast | `events.rs` |
| Updated main | `main.rs` (registration wiring) |
| Updated git hooks | `git_hooks.py` (HTTP instead of socket) |
| Updated loops | All loop modules (event broadcasts) |
| Updated store | `store/{mod,sqlite,postgres}.rs` (native types) |
| Updated infra | `Dockerfile`, `docker-compose.yml`, `README.md` (removed gRPC, updated port) |
| Removed | `server.rs`, `proto.rs`, `build.rs` (gRPC) |
| Roadmap updates | Concerto phase docs, auth/mobile designs |

---

## Verification

```bash
# WebSocket test
websocat ws://localhost:2486/ws

# API test
curl http://localhost:2486/api/waves
curl http://localhost:2486/health

# All tests pass
cargo test && cargo clippy -- -D warnings && uv run pytest tests/
```
