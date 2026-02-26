# Review: iOS Foreground Reconnect + Cross-Client Action Clearing

Branch: `jack-heart.mobile.20260226_1220`
Stage: 03-multi-client (now complete)

## What was implemented

iOS foreground reconnect, cross-client suggested action clearing, and a visual verification script. Three changes that close out Stage 03:

1. **Foreground reconnect.** When the app returns from background, `MobileRootView` calls `repoState.checkConnectionHealth()` which fast-tracks WebSocket reconnection (skipping stale backoff) and verifies the HTTP layer via `GET /status`. `MobileWaveDetailView` restarts output and session streams so the user sees current data.

2. **Cross-client action clearing.** `SessionState.reduce()` now calls `clearSuggestedActions()` on `.turnStarted` — when any client starts a turn, stale action buttons disappear on all connected clients.

3. **Verification script.** `scripts/verify_mobile_stage03.py` builds for iPhone Simulator and prints a manual checklist for action rail spacing, foreground reconnect, and cross-client clearing.

## Key choices

**Collapsed `handleNetworkRestored` into `resumeFromBackground`.** The design doc proposed keeping them separate, but the compress pass correctly identified they're identical. NWPathMonitor and foreground resume both call the same public method now.

**Stream restart uses existing lifecycle methods.** Rather than adding a new `SessionState.resumeFromBackground()`, the foreground handler calls `onDisappear()` then `onAppear()` — the same lifecycle pattern already used by `.task` and `.onDisappear` in the view. No new abstractions.

**Brief visual flash accepted.** `startStreaming` clears the buffer and replays from server. This causes a momentary content flash on foreground resume. The alternative — silently trusting stale output — is worse.

## How it fits together

```
iOS foreground (scenePhase → .active)
  │
  ├── MobileRootView
  │     └── repoState.checkConnectionHealth()
  │           ├── eventService.resumeFromBackground()  ← cancel backoff, retry WS now
  │           └── waveService.checkConnection()        ← GET /status, mark disconnected on failure
  │
  └── MobileWaveDetailView
        ├── outputBuffer.stop/start(waveId:)           ← clear + replay output
        └── sessionState.onDisappear/onAppear()        ← reset + replay session stream
```

EventService's existing reconnect loop, ConnectionBanner, and NWPathMonitor all continue to work unchanged. The foreground handler just skips the stale backoff delay.

## Risks and bottlenecks

**No automated test for foreground reconnect.** The behavior depends on iOS scenePhase transitions which can't be tested in `swift test`. The verification script covers manual validation. The underlying methods (`resumeFromBackground`, `checkConnection`, `clearSuggestedActions`) are all exercised by existing tests.

**`checkConnection()` hits the network on every foreground.** This is a single lightweight `GET /status` — sub-millisecond on local, ~50ms on Tailscale. Not a concern in practice since iOS foreground transitions are infrequent.

## What's not included

- macOS behavior changes (not needed — NWPathMonitor + local daemon suffice)
- Discovery / Stage 04 (design exists in `scratch/mobile-foreground-reconnect.md` Parts 2-4, not implemented yet)
- Background task registration or background fetch
- New connection states or enums

## Files changed

| File | Change |
|------|--------|
| `swift/LoopflowCore/Services/LocalEventService.swift` | `handleNetworkRestored` → `resumeFromBackground` (public) |
| `swift/LoopflowCore/State/RepoState.swift` | Added `checkConnectionHealth()` |
| `swift/LoopflowCore/State/SessionState.swift` | Added `clearSuggestedActions()` on `.turnStarted` |
| `swift/Concerto/Platform/iOS/MobileRootView.swift` | scenePhase observer → health check |
| `swift/Concerto/Platform/iOS/MobileWaveDetailView.swift` | scenePhase observer → stream restart |
| `scripts/verify_mobile_stage03.py` | Visual verification script |
| `wave/mobile/03-multi-client.md` | Status → complete, all items shipped |

## Test results

```
swift test --package-path swift          ✅
cargo test --all                         ✅
cargo clippy -- -D warnings              ✅
uv run pytest python/tests/              ✅
```
