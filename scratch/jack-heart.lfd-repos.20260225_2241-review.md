# Connections Panel — Design Review

## What was implemented

Provider auth cards in Concerto (macOS + iOS) that let users connect GitHub, Claude, and Codex from the UI. The full vertical: model layer (`AuthProvider`, `AuthFlow`, `AuthProviderStatus`), service layer (`/v0/auth` HTTP endpoints in `LocalWaveService`), state management (`AuthProviderStore` with real-time event reconciliation), and view layer (`AuthProviderCard` with copy-to-clipboard, URL fallback, and per-provider error display).

On the Rust side, `ClaudeAuthBroker` was refactored from filesystem heuristics (`~/.claude` directory scanning) to CLI-based auth (`claude auth status` / `claude auth logout`), matching how GitHub and Codex already work.

## Key choices

**CLI-driven auth over filesystem heuristics.** All three providers now use their CLI for status checks and disconnect. The prior Claude implementation scanned `~/.claude` for "auth-like" file names — fragile and tied to internal file layout. The CLI approach is the contract the providers actually support.

**Protocol-based service abstraction.** `AuthProviderStore` depends on `AuthProviderService` (a three-method protocol), not `LocalWaveService` directly. This enables test doubles without reshaping production code. The mock is an actor with canned responses — no factory patterns or dependency injection frameworks.

**CodingKeys for JSON mapping.** `AuthFlow` uses Swift camelCase properties (`verificationURI`, `userCode`) with `CodingKeys` for snake_case JSON wire format. The prior version used snake_case property names directly, which violated Swift naming conventions and made the API inconsistent with every other Swift type in the project.

**Event-driven state reconciliation.** `AuthProviderStore.handleEvent(_:)` processes WebSocket auth events (flow_started, connected, failed, disconnected) to keep UI state in sync with server-side changes — including flows started from the CLI or another client. On reconnect, a full refresh is triggered to reconcile any drift.

**409 conflict handling.** When `startAuthFlow` returns 409 (flow already pending), the store refreshes from the server and keeps the provider in pending state rather than surfacing an error.

## How it fits together

```
AuthProviderCard (View)
    ↓ callbacks
AuthProviderStore (State, @MainActor @Observable)
    ↓ calls
WaveService.listAuthProviders / startAuthFlow / disconnectProvider (HTTP)
    ↓ requests
lfd /v0/auth endpoints
    ↓ delegates
ProviderAuthService → GhAuthBroker / ClaudeAuthBroker / CodexAuthBroker (CLI)
```

Real-time updates flow back through the WebSocket event stream: `EventService` → `LFDEvent.auth` → `AuthProviderStore.handleEvent(_:)`.

## Risks and bottlenecks

- **CLI output parsing.** Auth brokers parse stdout/stderr from `gh`, `claude`, and `codex` CLIs. Any change to their output format could break URL/code extraction. Mitigated by defensive parsing (regex with fallbacks) and the fact that these CLIs are relatively stable.
- **Browser launch reliability.** The flow opens a browser for OAuth device flow. If the browser doesn't open, there's a fallback URL display with copy button — but the UX degrades. The `showURLFallback` flag on `AuthProviderCard` handles this.
- **Long timeouts on startAuthFlow.** The POST to `/v0/auth/{provider}` uses 30s request / 60s resource timeouts because the server spawns a CLI process and waits for URL extraction. This is correct but means the UI shows a pending state for potentially 30+ seconds if the CLI is slow.

## What's not included

- **Repo onboarding** (`POST /v0/repos`) — next wave item. The connections panel only manages provider auth, not repo registration.
- **Token refresh / expiry polling.** The `expired` status exists in the model but no automatic re-auth is triggered. Users must manually reconnect.
- **Accessibility on TextFields.** iOS/macOS connection setup forms lack `.accessibilityLabel()` on text inputs. Not a regression (these views existed before), but worth noting for a follow-up pass.
