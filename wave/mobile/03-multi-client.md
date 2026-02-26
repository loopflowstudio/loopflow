# 03: Multi-Client

Multiple devices connect to the same lfd. See the same waves, same sessions, same output.

Builds on multiplatform (shipped) — LoopflowCore holds shared state, iOS has purpose-built views, manual remote connection works. iOS action button wiring and concurrent-client backend coverage (`tests/e2e/test_concurrent_clients.py`) are already shipped.

## What needs work

### iOS foreground reconnect and stream restart

On iOS foreground resume, verify connection health and restart active streams:

- Add `scenePhase` handling in `MobileRootView` to call `await repoState.checkConnectionHealth()` when the app becomes active.
- Add `RepoState.checkConnectionHealth()` to ping `GET /health` for connected remotes and mark `.disconnected(.networkUnavailable)` on failure (to trigger existing reconnect machinery).
- Add `scenePhase` handling in `MobileWaveDetailView` to call `outputBuffer.startStreaming(waveId:)` and `await sessionState.onAppear()` on foreground.

### Cross-client suggested action clearing

Clear stale suggested actions when any client starts a new turn:

```swift
case .turnStarted(let turnId):
    currentTurnId = turnId
    turnState = .running
    clearSuggestedActions()
```

This keeps action buttons in sync when input comes from another connected client.

### Manual iOS visual verification script

Add `scripts/verify_mobile_stage03.py` to run iOS simulator setup/build and print a manual checklist for:

- iPhone 17 Pro action rail spacing + keyboard overlap
- iPad Pro 13-inch (M5) adaptive action layout + tap ergonomics
- background/foreground reconnect recovery
- cross-client action clearing behavior (two-device validation)

## Constraints

- No lfd API changes unless concurrent clients are actually broken
- No client registration, device tracking, or session awareness
- Mac client behavior must not change
- No studio/Tailscale discovery flow changes in this stage

## Done when

- Mac and iPhone Concerto connected to same lfd simultaneously
- Both see the same wave list and status updates in real time
- Starting a wave on Mac shows output on iPhone
- Chat transcript visible on both devices, messages from either device appear on both
- Action buttons appear on both devices; tapping on one clears stale suggestions on the other
- iOS action rail is manually verified on iPhone 17 Pro and iPad Pro 13-inch (M5)
- iPhone reconnects gracefully after backgrounding
