# 05: Concerto Remote Connection

## Problem

Concerto is hardcoded to `127.0.0.1:2486`, so it cannot control a remote lfd even though the protocol is already HTTP + WebSocket and lfd auth for remote is shipped.

Who benefits:
- Developers running lfd on a remote Linux box (EC2, homelab, always-on machine)
- Teams who want one daemon with persistent waves and logs
- Anyone who wants Concerto as a thin control plane, not a local-process UI

Why now: Phase 03 (static token auth) and Phase 04 (TLS via Caddy on `:443`) already unlocked server-side prerequisites. Phase 05 is the client wiring that makes remote real.

## Approach

Ship a **connection stack** in Concerto with one active target (local or remote), auth-aware HTTP/WS clients, TOFU certificate pinning, and resilient reconnect behavior.

### 1) Add a first-class connection model

Create `ServerConnection` in `LoopflowCore`:
- `host`, `port`, `useTLS`, `authMode`, `staticToken` (optional)
- Computed `httpBaseURL`, `wsBaseURL`
- `isLocal`: convenience flag (`host == "127.0.0.1" && !useTLS && authMode == .none`). Used **only** for timeout tier selection — never for auth or TLS decisions. A connection to `127.0.0.1:443` with TLS and a token is not local.
- `.local` default (`127.0.0.1:2486`, no auth, no TLS)

Persist non-secret fields in `UserDefaults`. Store `staticToken` in Keychain keyed by `host:port`.

### 2) Parameterize services by connection

Refactor:
- `LocalWaveService(connection:tokenProvider:sessionFactory:)`
- `LocalEventService(connection:tokenProvider:sessionFactory:)`

Both services must:
- Build requests from `connection`
- Inject `Authorization: Bearer <token>` when `authMode != .none` (driven by the field, not by host)
- Use `useTLS` to select `https`/`wss` scheme (driven by the field, not by host)
- Use timeout tiers based on `isLocal`:
  - `isLocal` (native daemon): request 3s / resource 10s
  - Otherwise: request 10s / resource 30s

`RepoState` becomes connection-driven: rebuilding wave/event services when the active connection changes.

### 3) Implement TOFU TLS pinning (fail closed on cert change)

For any TLS connection (`useTLS == true`, regardless of host):
- On first successful TLS handshake, store server certificate fingerprint (`SHA-256`) for `host:port`
- On future connects, require exact fingerprint match
- If fingerprint changes: mark connection `trustMismatch`, block requests/events, show "Trust New Certificate" flow

This follows SSH known_hosts semantics (TOFU + pinning), but with explicit UI approval on mismatch.

**Localhost caveat:** Caddy's `tls internal` regenerates self-signed certs on container recreation. For `127.0.0.1` with TLS, pin mismatch after `docker compose down && up` is expected. The connection settings UI should include a "Clear Pinned Certificate" action (per `host:port`) so developers aren't blocked during container cycling. This is a UX convenience, not a security relaxation — the user still explicitly re-trusts.

### 4) Harden WebSocket reliability for WAN

Replace fixed reconnect loop with exponential backoff + jitter:
- `1s, 2s, 4s, 8s, 16s, 30s cap`
- Reset backoff on successful `connected`
- Immediate retry when `NWPathMonitor` reports network restored

Expose connection state to UI (`connecting`, `connected`, `reconnecting`, `authFailed`, `trustRequired`, `disconnected(error)`), not just a boolean.

### 5) Add connection settings UI

Add a `ConnectionSettingsView` (sheet/menu):
- Host
- Port
- Use TLS toggle
- Auth mode (`none`, `static token`) for Phase 05
- Secure token field
- Connect/Test button

UI behavior:
- Show active server in sidebar header
- Show explicit disconnected reason and retry
- Keep one-click return to local mode

### 6) Add remote repo discovery endpoint (minimal lfd expansion)

To avoid manual server path entry, add `GET /v0/repos` in lfd returning unique repo roots from stored waves:
- `path`
- `name` (basename)
- `wave_count`

Concerto flow:
1. Connect to server
2. Fetch repos
3. User picks repo
4. Use selected **remote repo path** for all wave/worktree APIs

If zero repos exist, show a clear empty state with the exact command to bootstrap first remote wave (`lfq create <name> <repo>` on the server).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep local-only services, add a single “remote URL override” string | Quick patch, low code churn | Too fragile: no auth abstraction, no TLS trust model, no room for Studio auth in Phase 07 |
| Build full multi-server manager now (many concurrent daemons) | Future-proof and powerful | Too much scope for Phase 05; increases UI and state complexity before proving single-remote flow |
| Skip TOFU and let users disable TLS validation | Easiest networking path | Unacceptable security posture; enables silent MITM and breaks trust model for remote control |

## Key decisions

- **Decision: one active server at a time.**
  - Matches remote roadmap constraint: single-server now, daemon picker later.
  - Keeps state management tractable while shipping remote quickly.

- **Decision: TOFU pinning is mandatory for remote TLS.**
  - We will not offer “insecure skip verify.”
  - First connect stores trust; cert rotation requires explicit user re-trust.

- **Decision: connection state is explicit, not boolean.**
  - Avoids opaque “red dot” failures.
  - Lets users distinguish auth failure vs network outage vs cert mismatch.

- **Decision: auth and TLS are driven by explicit fields, not by host.**
  - `useTLS` and `authMode` control transport and auth. `isLocal` is only a latency hint for timeouts.
  - `127.0.0.1:443` with TLS + static token must work identically to `remote-host:443` with TLS + static token.
  - This makes the local Docker Compose stack (`docker-compose.prod.yml`) the primary dev test target.

- **Decision: repo selection uses server paths from lfd, never local filesystem paths.**
  - Prevents subtle path mismatch bugs when client and daemon run on different machines.

- **Principles followed from `wave/remote/README.md`:**
  - “The protocol doesn't change — only the host and transport.”
  - “Single server” for this phase.

- **Wild success rehearsal:**
  - Users add a remote host once, then forget about transport details.
  - Temporary network drops self-heal; logs/events resume without manual reconnect.
  - Teams treat Concerto as a reliable dashboard for long-running remote waves.

- **Wild failure rehearsal (and guardrails):**
  - Failure: silent cert changes accepted -> security incident. Guardrail: fail-closed pin mismatch.
  - Failure: reconnect storms under packet loss. Guardrail: bounded exponential backoff + jitter.
  - Failure: users think daemon is broken when token expired. Guardrail: explicit auth error state and message.

## Scope

- In scope:
  - `ServerConnection` model + persistence
  - Auth header injection for HTTP + WebSocket
  - TLS TOFU certificate pinning
  - Reconnect/backoff/network restoration handling
  - Connection settings UI (host/port/TLS/token)
  - lfd `GET /v0/repos` + Concerto repo picker
  - Clear disconnected/error states with retry
  - Local mode remains default and fully functional

- Out of scope:
  - Studio sign-in UX and JWT lifecycle (Phase 07)
  - Multi-server simultaneous monitoring
  - Offline caching/sync of wave state
  - Remote file browsing/editor integration (Phase 06/08)

## Done when

```bash
# 1) Start remote stack with TLS proxy and static auth
cd /Users/jack/src/loopflow.remote
docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml up -d

# 2) Run Swift package tests after implementation
swift test --package-path swift
```

Observable outcomes:
1. Concerto connects to `https://127.0.0.1:443` (local container) with static token and shows connected state.
2. Same connection works with `https://<remote-host>:443` — no code path differences.
3. Repo picker loads from `GET /v0/repos`; selecting a repo loads remote waves.
4. Create/run/stop/land/next operations succeed against remote lfd.
5. WebSocket events and log streaming update in real time after WAN reconnects.
6. Cert fingerprint mismatch blocks connection until explicit re-trust.
7. "Clear Pinned Certificate" unblocks after container recreation (cert regeneration).
8. Switching back to `.local` works without restarting the app.
