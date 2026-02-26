# Complete Stage 03: Multi-Client Reliability

## Problem

You're coding on your Mac with Concerto running. Your iPhone is connected to the same lfd (via Tailscale, coming in Stage 04). You lock your phone, come back 10 minutes later — the session stream is dead, output stopped, and the action buttons from 10 minutes ago are still showing even though the Mac already acted on them.

Stage 03 makes the client-side experience reliable for dual-device use. No connectivity changes (that's Stage 04) — this is about what happens after you're already connected.

## Approach

### 1. iOS foreground reconnect

**The gap:** EventService has robust reconnect (exponential backoff + `NWPathMonitor`), but nothing triggers a check when iOS returns from background. The WebSocket may have been silently killed by the OS during suspension. macOS has `scenePhase` monitoring in `WaveDetailPanel` that refreshes wave content on foreground — iOS has nothing equivalent.

**The fix:** Add `scenePhase` monitoring in `MobileRootView`:

```swift
@Environment(\.scenePhase) private var scenePhase

.onChange(of: scenePhase) { _, phase in
    guard phase == .active else { return }
    Task { await repoState.checkConnectionHealth() }
}
```

`RepoState.checkConnectionHealth()` is a new lightweight method:
- If not connected: no-op (auto-connect or ConnectionSetupView handles this)
- If connected: ping `GET /health` on the active connection
- If health check fails: set `connectionState = .disconnected(.networkUnavailable)`, which triggers EventService's existing reconnect logic
- If health check succeeds: no-op (connection is fine, WebSocket is still alive or will reconnect on its own)

This is simpler than threading `scenePhase` into EventService. One health check on foreground, let existing machinery handle failures.

**OutputBuffer stream restart:** OutputBuffer tracks active streams via `streamTasks` keyed by wave ID, but tasks get cancelled when the OS suspends the app. On foreground resume, the currently-viewed wave's output stream needs to restart.

In `MobileWaveDetailView`, add `scenePhase` handling:

```swift
@Environment(\.scenePhase) private var scenePhase

.onChange(of: scenePhase) { _, phase in
    guard phase == .active else { return }
    outputBuffer.startStreaming(waveId: wave.id)
    Task { await sessionState.onAppear() }
}
```

`outputBuffer.startStreaming()` already prevents duplicate streams (checks `streamTasks`), so calling it again is safe — it'll restart if the task was cancelled, no-op if still running. `sessionState.onAppear()` already handles replay from `lastAppliedSeq`.

**Platform boundary:** `scenePhase` handlers live in `Concerto/Platform/iOS/` views. No `#if os(iOS)` in shared code. The reconnect logic in RepoState, EventService, OutputBuffer, and SessionState stays platform-agnostic.

### 2. Cross-client suggested action clearing

**The bug:** `SessionState.reduce()` handles `.turnStarted` by setting `currentTurnId` and `turnState = .running`, but doesn't clear suggested actions. When client A sends a message:
1. Client A calls `send()` → clears actions locally → posts to server
2. Server starts a new turn, broadcasts `.turnStarted` to all subscribers
3. Client B receives `.turnStarted` via SSE — but actions aren't cleared

Client B keeps showing stale action buttons until the agent sends new suggestions or the turn completes.

**The fix:** One line in `reduce()`:

```swift
case .turnStarted(let turnId):
    currentTurnId = turnId
    turnState = .running
    clearSuggestedActions()  // ← add this
```

This is correct for both local and remote sends:
- **Local send:** already cleared in `send()`, redundant clear is harmless
- **Remote send (from other client):** `.turnStarted` arrives via SSE, clears stale actions

No new event types. No server changes. The `.turnStarted` event is already the definitive signal that input was accepted — it covers all input paths (text, suggested action, future input types).

### 3. Visual verification script

Write `scripts/verify_mobile_stage03.py` that:
1. Builds Concerto for iOS simulators (iPhone 17 Pro, iPad Pro 13-inch M5) using `xcodebuild`
2. Boots the simulators and installs the app
3. Prints a checklist of what to verify:
   - Bottom action rail: thumb-zone spacing on iPhone, no keyboard overlap
   - Action buttons: adapt to iPad width, tap targets meet 44pt minimum
   - Reconnect: background the app (Home button), wait 10 seconds, return — output and session resume
   - Action clearing: (requires two connected clients) tap action on one, verify clear on other

One command: `uv run python scripts/verify_mobile_stage03.py`

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Thread `scenePhase` through EventService | More targeted reconnect | Adds iOS awareness to shared code. Health check from the view layer is simpler and keeps EventService platform-agnostic. |
| Clear actions on `userMessage` event | More precise trigger | Requires distinguishing "my message" from "other client's message" — unnecessary complexity. `.turnStarted` covers all cases. |
| `UIApplication.didBecomeActiveNotification` | Traditional UIKit approach | SwiftUI's `scenePhase` is the idiomatic equivalent and works with the existing view lifecycle. No need to mix UIKit notifications in. |
| Full automated UI test suite | Regression guard for visual issues | Snapshot testing is brittle across Xcode versions and simulator updates. A runnable script with a human checklist is more reliable and faster to maintain. |
| Mobile access toggle + QR code pairing | Connect without Tailscale/studio | Binding to `0.0.0.0` repeats the OpenClaw security mistake. Stage 04's Tailscale + studio discovery is the right connectivity path. |

## Key decisions

**Health check on foreground, not WebSocket ping.** When iOS returns from background, the WebSocket state is unknown — it may have been silently killed. A single `GET /health` is definitive. If it fails, EventService's existing exponential backoff + `NWPathMonitor` handles recovery. If it succeeds, everything is fine. This avoids adding platform awareness to EventService.

**Clear on `.turnStarted`, not on user message.** Turn start is the definitive signal that input was accepted and the agent is working. Clearing on user message receipt would require tracking message authorship to distinguish local from remote — unnecessary for the desired behavior.

**No connectivity changes in Stage 03.** Research into OpenClaw, Signal, Plex, and Home Assistant confirms that secure remote connectivity requires either a central account service (studio) or a mesh VPN (Tailscale). Both are Stage 04 scope. Stage 03 makes the client reliable so Stage 04 can focus on connectivity.

**Script over snapshot tests for visual verification.** Snapshot tests break on Xcode updates and don't catch ergonomic issues (thumb reach, keyboard overlap). A human looking at a simulator with a checklist catches what automated tests can't.

## Scope

- **In scope**: `scenePhase` foreground reconnect in MobileRootView and MobileWaveDetailView. `RepoState.checkConnectionHealth()` method. Cross-client action clearing in `SessionState.reduce()`. OutputBuffer stream restart on foreground. Visual verification script.
- **Out of scope**: Connectivity / pairing (Stage 04). `0.0.0.0` binding (rejected — security risk). QR code pairing (deferred — Tailscale + studio is the right path). lfd changes. Multiple saved connection profiles.
- **Related**: `scratch/mobile-verify-concurrent-clients.md` — e2e test proving server-side broadcast works. This design is the client-side complement.

## Done when

```bash
# Swift tests pass
swift test --package-path swift

# Visual verification
uv run python scripts/verify_mobile_stage03.py
```

Observable outcomes:
- iPhone Concerto: background app for 30 seconds, return — output stream and session resume within 3 seconds, no missing events
- Mac: tap action button on a shared session — iPhone's stale suggestions clear within 1 second (requires two clients connected to same lfd)
- iOS action rail verified on iPhone 17 Pro (thumb zones) and iPad Pro 13-inch M5 (adaptive layout)

**Wave goals advanced:**
- "iPhone reconnects gracefully after backgrounding" (stage 03 done-when)
- "Action buttons appear on both devices; tapping on one clears stale suggestions on the other" (stage 03 done-when)
- "iOS action rail is manually verified on iPhone 17 Pro and iPad Pro 13-inch (M5)" (stage 03 done-when)
