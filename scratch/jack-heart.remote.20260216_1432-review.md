# Remote Concerto (Phase 05) — Review Guide

## What was implemented

- Added first-class remote connection modeling in LoopflowCore (`ServerConnection`, `ConnectionState`, `RepoTarget`, `RemoteRepo`).
- Refactored HTTP/WebSocket clients to be connection-driven (`WaveService`, `EventService`) with:
  - TLS-aware URL building (`http/https`, `ws/wss`)
  - auth header injection for static-token mode
  - local-vs-remote timeout tiers
- Added TLS TOFU certificate pinning with fail-closed mismatch handling and explicit trust/reset actions.
- Added deterministic connect/test handshake in Concerto (`tlsTrustCheck -> authCheck -> repoDiscovery -> wsProbe`) and mapped failures to explicit UI states.
- Added WAN-friendly WebSocket reconnect behavior with exponential backoff, jitter, and immediate retry on network restore.
- Added `GET /v0/repos` in lfd and wired remote repo selection in Concerto.
- Added connection settings UI (host/port/TLS/auth/token, connect/test, trust actions, switch-to-local).
- Added correctness guards so local-only filesystem actions are disabled in remote mode.

## Key choices

- **Single active connection model**: keeps state manageable while enabling real remote control now.
- **TOFU pinning required for TLS**: no insecure bypass path; cert changes require explicit user trust.
- **Connection state as enum, not boolean**: makes auth/network/trust failures visible and actionable.
- **Repo selection by server path (`RepoTarget.remote`)**: prevents local-path assumptions against remote daemons.
- **Compatibility aliases retained (`LocalWaveService` / `LocalEventService`)**: allows incremental migration while new neutral types (`WaveService`, `EventService`) are primary.

## How it fits together

`RepoState` now drives all transport through `ConnectionStore` + connection-aware services. Connect/Test runs explicit handshake phases, then starts event subscription and repo-scoped wave loading. `WaveService` handles HTTP operations (including `/v0/repos`), `EventService` handles WebSocket events and reconnect policy, and certificate pinning is enforced by a shared URLSession delegate/store keyed by `host:port`.

## Risks and bottlenecks

- **TOFU first-connect trust**: first cert is trusted as-is; this is expected by design but should be validated against threat model.
- **Connection orchestration split**: persistence/trust storage is in `ConnectionStore`, while handshake/reconnect orchestration is still largely in `RepoState`.
- **Remote capabilities are intentionally partial**: local Finder/terminal/editor actions are gated off in remote mode rather than replaced with remote-aware equivalents.
- **UI validation gap previously present**: now mitigated by host/port/token validation in Connection Settings.

## What's not included

- Multi-server/session management.
- Studio/JWT auth lifecycle.
- Full remote file access and remote typeahead/file browsing APIs.
- Remote terminal/editor launch flows (only local action gating shipped).
- Extra UI polish beyond correctness-critical connection state messaging.
