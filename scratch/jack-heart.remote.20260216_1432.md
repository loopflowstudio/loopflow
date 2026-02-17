# Remote Concerto (Phase 05) — Current State

## Goal
Enable Concerto to control local or remote `lfd` through a connection-driven transport stack (HTTP + WebSocket), with explicit auth/TLS settings, TLS pinning, repo discovery, and resilient reconnect behavior.

## What shipped on this branch

- Added first-class remote connection models in LoopflowCore:
  - `ServerConnection`
  - `ConnectionState`
  - `RepoTarget`
  - `RemoteRepo`
- Refactored transport services to be connection-driven (`WaveService` / `EventService` behavior via existing compatibility aliases):
  - URL scheme selected by connection (`http/https`, `ws/wss`)
  - auth header injection when auth mode requires a token
  - local vs remote timeout tiers based on `isLocal`
- Implemented TLS TOFU certificate pinning with fail-closed mismatch handling.
- Added explicit trust actions:
  - trust new certificate
  - clear pinned certificate for a specific `host:port`
- Added deterministic connect/test handshake in Concerto:
  - `tlsTrustCheck -> authCheck -> repoDiscovery -> wsProbe`
- Added WAN-friendly WebSocket reconnect behavior:
  - exponential backoff with jitter (capped)
  - immediate retry on network restore
- Added `GET /v0/repos` to `lfd` and wired remote repo selection in Concerto.
- Added connection settings UI (host/port/TLS/auth/token, connect/test, trust/reset, switch back to local).
- Added correctness guards so local-only filesystem actions are disabled in remote mode.

## Final design decisions retained

- Single active server connection for this phase.
- No insecure TLS bypass; pin mismatch requires explicit user action.
- Connection state is explicit (not a boolean), so auth/trust/network failures are distinguishable.
- Auth and TLS are controlled by explicit connection fields, not inferred from host.
- Remote repo selection uses server paths (`RepoTarget.remote(path:)`), never local filesystem assumptions.

## Known limits (intentional for Phase 05)

- No multi-server/session management.
- No Studio/JWT auth lifecycle.
- No full remote file browsing/typeahead parity.
- No remote terminal/editor launch flow (only local-action gating).

## Follow-up candidates

- Move full handshake/reconnect orchestration ownership from `RepoState` into `ConnectionStore`.
- Expand remote capabilities (file access/typeahead, remote launch actions).
- Add multi-server management and richer diagnostics.

## Validation focus

- `swift test --package-path swift`
- Targeted lfd coverage for `GET /v0/repos`
- Manual local TLS+token flow via compose/Caddy
- Remote-host parity checks for connect/load/run/event updates

## Open questions

Tracked in `scratch/questions.md`.
