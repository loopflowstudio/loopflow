# luna-verse: lfd gRPC → HTTP+WebSocket + loopflow.studio registration

## What was implemented

Replaced lfd's gRPC transport layer with HTTP REST + WebSocket, consolidated to a single port (2486), and added loopflow.studio registration for mobile client discovery.

### Rust (lfd)

- **HTTP API** (`http.rs`): Full wave CRUD, run/stop/land operations, git hooks, health/status/metrics endpoints via Axum
- **WebSocket streaming** (`http.rs`): Live event subscriptions replacing gRPC server streaming
- **Registration** (`registration.rs`): lfd registers with loopflow.studio on startup, heartbeats every 60s, manages connection tokens for mobile client validation
- **Auth** (`auth.rs`): AuthContext with token validation and caching for remote connections
- **Types module** (`types/`): Pure Rust domain types (Wave, WaveRun, Agent, Stimulus, Event) replacing proto-generated code
- **Config** (`config.rs`, `credentials.rs`, `machine_id.rs`): YAML config loading, JWT credential management, machine identification
- **Deleted**: `server.rs` (gRPC server), `proto.rs`, `build.rs` (protobuf compilation), all tonic/prost dependencies

### Swift (Concerto)

- `RepoState.swift` now uses `LocalWaveService` (HTTP) and `LocalEventService` (Unix socket) — both already existed in LoopflowCore

### Roadmap

- Added Phase 2 mobile architecture docs (`02-studio-auth-v0.md`, `03-mobile-client-v0.md`)
- Added Phase 3 (chat) and Phase 4 (agent harness) roadmap items
- Updated existing items to reflect terminal streaming deferral

## Key choices

**Single port (2486)**: HTTP and WebSocket share one listener. Simpler for firewalls, Tailscale, Docker port mapping. gRPC's separate port was unnecessary given we only had unary RPCs and one streaming call.

**Unix socket for local events**: LocalEventService connects via `~/.lf/lfd.sock` instead of TCP. Lower latency for the local Concerto ↔ lfd connection. Remote mobile clients use WebSocket over HTTP.

**Registration with loopflow.studio**: Mobile clients discover lfd via a central registry rather than manual IP entry. lfd heartbeats to keep the registration fresh. Connection tokens validate that remote clients are authorized.

**Proto-free types**: New `types/` module defines Wave, WaveRun, Agent, etc. as plain Rust structs with serde derives. Removes the protobuf compilation step and proto file dependency.

## How it fits together

```
Mobile → loopflow.studio (discover lfd URL) → lfd HTTP API (port 2486)
Concerto → LocalWaveService (HTTP to localhost:2486)
         → LocalEventService (Unix socket ~/.lf/lfd.sock)
```

lfd starts HTTP server on 2486, optionally registers with loopflow.studio if credentials exist, and serves both local (Concerto) and remote (mobile) clients through the same API.

## Risks and bottlenecks

- **Registration availability**: If loopflow.studio is down, mobile can't discover lfd. Local Concerto is unaffected.
- **Token validation latency**: First remote request validates the connection token with loopflow.studio. Cached after that.
- **No HTTPS on local**: lfd serves plain HTTP on localhost. Remote access relies on Tailscale or similar for transport encryption.

## What's not included

- Mobile iOS app (Phase 2 — roadmap only)
- Chat interface (Phase 3 — roadmap only)
- Agent harness (Phase 4 — roadmap only)
- loopflow.studio server implementation (separate repo)
- Push notifications (documented but not built)
- HTTPS/TLS termination (expected to use Tailscale)
