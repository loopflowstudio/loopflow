# 03: Concerto Connections Panel

Status: **next**

Users can connect GitHub, Claude, and Codex through `lfq auth` in the terminal, but not from Concerto. This phase adds provider auth cards in Concerto so auth is browser-first and identical across clients.

## API contract already available

No server-side auth API work is required.

- `GET /v0/auth` → `{ providers: [{ provider, status, login? }] }`
- `GET /v0/auth/{provider}` → `{ provider, status, login? }`
- `POST /v0/auth/{provider}` → `{ provider, verification_uri, verification_uri_complete?, user_code?, expires_in? }`
- `DELETE /v0/auth/{provider}` → `{ provider, status, login? }`
- Events: `auth.flow_started`, `auth.connected`, `auth.failed`, `auth.disconnected`

## What to build

1. Add shared Swift auth models in `LoopflowCore` (`AuthProviderStatus`, `AuthFlow`).
2. Extend `LocalEventService` to parse `auth.*` events into `LFDEvent` cases.
3. Extend `LocalWaveService` with `/v0/auth` HTTP methods:
   - list statuses,
   - start auth flow,
   - disconnect provider.
4. Add `AuthProviderStore` state container:
   - refresh from `GET /v0/auth`,
   - connect/disconnect actions,
   - websocket event reconciliation.
5. Extend connection UIs:
   - macOS: `ConnectionSettingsView`
   - iOS: `ConnectionSetupView`
   - three provider cards (GitHub/Claude/Codex) with status, login, connect/disconnect, and pending user code.

## UX flow

- Connect:
  1. Call `POST /v0/auth/{provider}`.
  2. Open `verification_uri_complete` (fallback `verification_uri`) in browser.
  3. Show pending state + `user_code` if present.
  4. Reconcile to active/failed from websocket events.
- Disconnect:
  1. Call `DELETE /v0/auth/{provider}`.
  2. Update card state from response.

## Failure modes to handle

- Provider binary missing on host (`command not available`).
- Flow already pending (`409`).
- Browser launch failure (show copyable URL fallback).
- lfd reconnect mid-flow (refresh `/v0/auth` on reconnect).

## Done when

- Users can connect/disconnect GitHub, Claude, and Codex from Concerto with no terminal steps.
- Card state tracks `active` / `pending` / `none` from HTTP + websocket events.
- `cargo test --all` and `swift test --package-path swift` pass.
