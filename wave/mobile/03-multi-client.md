# 03: Multi-Client

Multiple devices connect to the same lfd. See the same waves, same sessions, same output.

**Status: in progress** (updated February 26, 2026)

## What to build

Multiple Concerto clients connect to the same lfd and see consistent state. No client awareness — just a shared view. Both clients see the transcript, both can send messages. Like two browser tabs open to the same chat.

Stage 03 assumes manual/direct connection is already working from Stage 01. Discovery UX is out of scope here (Stage 04).

**From Stage 02:** ChatState → SessionState, WaveChatView → WaveSessionView across Swift. Suggested actions flow through `SessionEvent::SuggestedActions` (server-side stream) → `SessionState.suggestedActions` (client-side). References below use the new names.

## What's already free

lfd owns all state. Concerto is a thin client. Most multi-client already works:

- Wave list: both clients poll/subscribe via the same HTTP + WebSocket APIs
- Wave status: EventService pushes updates to all connected WebSocket clients
- Output streaming: each client streams independently from `/v0/waves/{id}/output/stream`
- Chat transcript: stored server-side, replayed on connect
- Chat input: any client can send messages to the same session

No session awareness or handoff needed. Multiple clients just connect and see the same thing.

## What needs work

### ~~iOS action button wiring (pre-req)~~ shipped (February 25, 2026)

Done in `MobileWaveDetailView` with shared behavior parity:

- Suggested actions now render in a persistent bottom `.safeAreaInset(edge: .bottom)` rail on iOS.
- iOS chat tab disables duplicate action rendering/lifecycle by passing `showsSuggestedActions: false` and `managesLifecycle: false` into `WaveSessionView`.
- Action taps use the existing shared path: `await sessionState.sendSuggestedAction(action)`.
- Session lifecycle/context (`configureClientContext`, `onAppear`, `onDisappear`) is owned by the iOS detail container so actions stay live while users watch Output.

### ~~Verify concurrent clients work~~ shipped (February 26, 2026)

Added `tests/e2e/test_concurrent_clients.py` and CI coverage in the `e2e-smoke` job to verify:

- dual WebSocket wave event fanout
- dual wave log streaming
- dual SSE session event fanout
- chat input visibility across two subscribers
- `suggested_actions` event parseability in the Python client

Validation command:

```bash
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

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
