# 05: Concerto Remote Connection

## Problem

Concerto can already talk to local `lfd`, and remote auth + infrastructure are in place, but remote use is not yet a first-class path end-to-end.

We need one reliable WAN path for:

- Wave CRUD over HTTPS
- Run events over WSS (`/ws`)
- Chat turn events over SSE (`/v0/waves/:id/chat/events`)
- Live log streaming over HTTPS

Who benefits: teams running `lfd` on remote Linux hosts (EC2 or similar) while using Concerto locally.

Why now: this is the phase that turns remote from "possible" into "daily-driver usable".

## Approach

Ship a **transport-parity contract**: local and remote use the same client services and API shapes; only host, scheme, and auth mode change.

1. **Keep one client path in Concerto**
   - Continue using `WaveService` + `EventService` for both local and remote.
   - Remote is just `ServerConnection(host, port, useTLS=true, authMode=.staticToken)`.
   - No remote-specific forked service layer.

2. **Harden the remote connection handshake**
   - Keep phased connection checks: TLS trust check → auth check → repo discovery → WS probe.
   - Add explicit remote error mapping for daemon timeout/fail-fast so users see execution failures, not generic request errors.

3. **Make proxy streaming behavior explicit**
   - Keep Caddy as TLS terminator in front of `lfd`.
   - Validate both long-lived transports through Caddy in CI-like smoke coverage:
     - WSS for run events
     - SSE for chat events
   - Ensure streaming is immediate (no proxy buffering regressions) and terminal events close cleanly.

4. **Add remote transport smoke coverage**
   - Bring up compose + Caddy.
   - Exercise: create wave, run, receive WS events, start chat, receive SSE events, stream logs.
   - Fail the test if either stream stalls, downgrades, or drops auth headers.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build a separate “RemoteWaveService” client stack | Fast to isolate remote quirks | Duplicates logic and breaks protocol-parity goal |
| Move chat events from SSE to WebSocket only | Single transport to test | Unnecessary protocol change; higher risk and bigger scope than Phase 05 |
| Add filesystem mount/sync for remote workflows now | Could simplify some UX paths | Violates remote architecture direction; Phase 06 handles editor access via native remote tooling |

## Key decisions

- **Decision: protocol parity is non-negotiable.** We are following the wave principle: _"Keep protocol parity: Concerto talks to local and remote lfd via the same HTTP/WS surface."_ This avoids remote-only drift and cuts long-term maintenance cost.
- **Decision: correctness over convenience for streaming.** We will treat SSE + WSS through TLS proxy as release-blocking, with automated smoke coverage, not ad-hoc manual verification.
- **Decision: timeout/fail-fast errors get first-class UX copy.** Wild failure for this phase is users seeing “request failed” when the daemon actually timed out. We will map these errors explicitly so recovery actions are obvious.
- **Decision: keep scope tight to Phase 05.** We are following: _"Ship remote connectivity incrementally: secure auth first, then UX and API breadth."_ Remote IDE/file UX and JWT auth stay in later phases.

## Scope

- In scope:
  - HTTPS/WSS remote connection flow in Concerto using static token auth
  - Wave CRUD, WS run events, SSE chat events, and log streaming over WAN
  - Clear timeout/fail-fast error surfacing in Concerto
  - Proxy-path verification for both SSE and WSS
- Out of scope:
  - Remote editor/file actions (Phase 06)
  - Studio JWT auth/sign-in/discovery UX (Phase 07)
  - Broader API expansion (Phase 08)

## Done when

- Concerto connects to remote `lfd` over HTTPS/WSS and stays stable across reconnects.
- Wave CRUD, WS events, SSE chat events, and logs all work through Caddy TLS proxy.
- Daemon timeout/fail-fast errors appear as explicit user-facing messages (not generic transport failures).
- Remote transport smoke test passes with both stream types in one run (WSS + SSE).
