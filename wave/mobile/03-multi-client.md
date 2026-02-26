# 03: Multi-Client

Multiple devices connect to the same lfd. See the same waves, same sessions, same output.

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

### ~~Manual iOS visual verification for action rail~~ ingested → scratch/mobile-complete-stage-03.md

### ~~Suggested action consistency across clients~~ ingested → scratch/mobile-complete-stage-03.md

### ~~Connection UX on iOS~~ ingested → scratch/mobile-complete-stage-03.md

### ~~Reconnect resilience~~ ingested → scratch/mobile-complete-stage-03.md

### ~~Verify concurrent clients work~~ ingested → scratch/mobile-verify-concurrent-clients.md

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
