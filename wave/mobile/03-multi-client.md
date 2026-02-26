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

### iOS action button wiring (pre-req)

`ActionButtonsView` exists in LoopflowCore but isn't embedded in `MobileWaveDetailView` yet. Small change — wire it up early in Stage 03 before multi-client testing. Both devices should show the same features.

### Suggested action consistency across clients

Suggested actions are client-side ephemeral state in `SessionState`. When one client taps an action (sending a message), the other client needs to clear its stale suggestions. This should happen naturally: the session event stream carries the new user message, the other client's `SessionState` sees the turn change and clears. Verify this works — if session events don't propagate user messages to other clients in real time, stale action buttons will linger.

### Connection UX on iOS

iOS Concerto connects to a remote lfd (no local daemon). Needs:

- Connection setup screen (host, port, optional auth token) — *shipped in Stage 01 as ConnectionSetupView*
- Saved connection profiles (like SSH configs) — *revisit here if needed; Stage 01 tried ConnectionProfile and removed it — ConnectionStore with ConnectionMode (bundled/remote) was sufficient for single-connection use*
- Reconnect handling (mobile goes to background, comes back)

**Note from Stage 01:** ConnectionProfileStore was prototyped and removed. The current ConnectionStore handles one active connection. If multi-client needs multiple saved connections (e.g., switching between lfds), reintroduce profiles. If users only connect to one lfd at a time, the current model may suffice — just persist the last-used host:port.

### Reconnect resilience

Mobile clients disconnect frequently (background, network switch, lock screen). On foreground resume:

- EventService WebSocket: reconnect with last sequence number for gap-free replay
- Output streaming: reconnect to stream endpoint
- SessionState: already has reconnect logic (sequence numbers, replay) — verify it works with real network interruption

Hook into `UIApplication.didBecomeActiveNotification` on iOS:

```swift
#if os(iOS)
NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)
    .sink { _ in reconnect() }
#endif
```

### Verify concurrent clients work

lfd may not have been tested with multiple simultaneous WebSocket connections or concurrent session input. Verify:

- Two clients subscribed to the same wave's events both receive updates
- Two clients streaming the same wave's output both receive lines
- Two clients viewing the same chat session both see messages
- Sending from either client works without conflict

If lfd has single-client assumptions (e.g. one WebSocket per repo), fix those.

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
- iPhone reconnects gracefully after backgrounding
