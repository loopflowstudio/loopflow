# iOS Foreground Reconnect and Stream Restart

## Problem

When an iOS app is backgrounded, iOS suspends the process. WebSocket connections die silently, HTTP streams (output, session events) disconnect. When the user returns to the app, they see stale data and a dead connection. The existing `EventService` reconnect machinery works — exponential backoff, NWPathMonitor — but it's optimized for network transitions, not iOS foreground resume. The backoff timer was started when the connection dropped (possibly minutes ago), and the `Task.sleep` was suspended alongside the app. On resume, the user waits for whatever delay remained on a timer that was set for a different moment.

The user expects instant recovery: tap the app, see current state.

**Wave goal:** "iPhone reconnects gracefully after backgrounding" (03-multi-client done-when).

## Approach

Five focused changes that compose the existing connection and streaming infrastructure:

### 1. EventService: `resumeFromBackground()`

Add a public method that fast-tracks reconnection when the WebSocket is dead. Identical to the existing `handleNetworkRestored()` — cancel any pending backoff delay, retry immediately — but callable from outside.

```swift
// LocalEventService.swift
public func resumeFromBackground() async {
    guard !_isConnected else { return }
    reconnectTask?.cancel()
    reconnectTask = nil
    await retryNow()
}
```

If already connected, no-op. If in a reconnect loop, skip the delay and retry now. No new state machines, no new enums.

### 2. RepoState: `checkConnectionHealth()`

Two-step health check: fast-track the WebSocket reconnect, then verify HTTP connectivity for the case where the WebSocket appears fine but the HTTP layer is stale.

```swift
// RepoState.swift
public func checkConnectionHealth() async {
    // Fast-track WebSocket reconnection if disconnected
    await eventService?.resumeFromBackground()

    // Verify HTTP layer for ostensibly-connected state
    if connectionState == .connected {
        do {
            try await waveService.checkConnection()  // GET /status
        } catch {
            updateConnectionState(.disconnected(.networkUnavailable))
        }
    }
}
```

Uses the existing `waveService.checkConnection()` which hits `GET /status`. On failure, marks `.disconnected(.networkUnavailable)` — the ConnectionBanner immediately shows the error state, and EventService's reconnect loop picks up recovery.

### 3. MobileRootView: scenePhase observer

Trigger the health check whenever the app returns to foreground.

```swift
// MobileRootView.swift
@Environment(\.scenePhase) private var scenePhase

// On the outer Group:
.onChange(of: scenePhase) { _, phase in
    guard phase == .active, !needsInitialSetup else { return }
    Task {
        await repoState.checkConnectionHealth()
    }
}
```

Fires once on foreground. Skips if the user hasn't connected yet (`needsInitialSetup`). The health check is async and non-blocking — the UI remains responsive.

### 4. MobileWaveDetailView: scenePhase observer

Restart output streaming and session event streaming when the user returns to a wave detail view.

```swift
// MobileWaveDetailView.swift
@Environment(\.scenePhase) private var scenePhase

// Inside detailContent(for:), chained after .onDisappear:
.onChange(of: scenePhase) { _, phase in
    guard phase == .active else { return }
    outputBuffer.stopStreaming(waveId: wave.id)
    outputBuffer.startStreaming(waveId: wave.id)
    sessionState.onDisappear()
    Task {
        await sessionState.onAppear()
    }
}
```

**Output stream:** Stop then start. `startStreaming` clears the buffer and replays from the server — the output view refreshes with current data. Brief content flash is acceptable on foreground resume.

**Session stream:** `onDisappear()` cancels the stale stream task and resets phase. `onAppear()` reconnects to the session event stream, replaying from `lastAppliedSeq` if available (so the user doesn't see duplicate messages). This matches the existing lifecycle pattern used by `.task` and `.onDisappear` in the same view.

### 5. macOS unchanged

The macOS `WaveDetailPanel` already has a scenePhase handler that refreshes wave content on foreground. No changes needed. EventService's existing NWPathMonitor + reconnect loop handles macOS recovery (macOS doesn't aggressively suspend apps like iOS does).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Full `connectLfd` handshake on foreground | Guarantees fresh state — TLS check, auth check, repo discovery, WS probe, event subscription restart | Tears down everything, resets `sessionStates`, clears wave list. Heavy and visible — the UI flickers, session context is lost. Overkill when the connection just needs a nudge. |
| Do nothing (rely on EventService reconnect) | Zero code changes. EventService already has backoff + NWPathMonitor. | Backoff timer was set when the connection dropped, potentially minutes ago. On foreground, the user waits for whatever delay remained — could be 15-30 seconds. NWPathMonitor doesn't fire on iOS foreground because the network path didn't change (app was suspended, not network). |
| Add periodic keepalive pings | Detect connection death faster, reduce stale state window | iOS suspends background tasks — keepalive pings won't run. And when the app is active, EventService's receive loop already detects failures immediately. Solves a problem that doesn't exist. |
| Observe `connectionState` transitions in detail view to restart streams | Cleaner separation — streams restart when connection is confirmed healthy, not on a timer | Over-couples stream lifecycle to connection state transitions. Streams would restart on every reconnect (including transient network blips), not just foreground resume. The current approach (foreground = restart) is simpler and more predictable. |

## Key decisions

**`resumeFromBackground` is a separate method, not a refactor of `handleNetworkRestored`.** They serve different triggers (foreground resume vs. NWPathMonitor) but do the same thing internally. Keeping them separate avoids coupling the two paths and makes the intent clear at each call site.

**Output buffer clears on restart.** `startStreaming` replays from the server, so clearing prevents duplicates. The brief visual flash is the right tradeoff — stale output that silently disagrees with the server is worse than a momentary refresh.

**Session stream uses `onDisappear` + `onAppear`, not a new method.** Adding `SessionState.resumeFromBackground()` would hide the same two calls behind a name. The lifecycle pair is already the canonical pattern in MobileWaveDetailView — `.task` calls `onAppear`, `.onDisappear` calls `onDisappear`. The foreground handler follows the same pattern.

**No debouncing or throttling.** iOS fires `scenePhase` changes infrequently (only on actual foreground/background transitions). Multiple rapid transitions don't happen in practice, and the health check is idempotent.

## Scope

- **In scope:** scenePhase handling in MobileRootView and MobileWaveDetailView. EventService public reconnect nudge. RepoState health check method. HTTP health ping on foreground.
- **Out of scope:** macOS behavior changes. New connection states or enums. lfd API changes. Session awareness or device tracking. Discovery flow changes. Background task registration or background fetch.

## Done when

```bash
# Automated: existing concurrent-client tests still pass
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v

# Swift: package and UI tests pass
swift test --package-path swift
```

Manual verification (covered by Stage 03 visual verification script, but can be checked independently):

1. Connect iPhone Simulator to a running lfd
2. Open a wave detail view with live output
3. Background the app (Cmd+Shift+H in Simulator)
4. Wait 10+ seconds
5. Foreground the app
6. **Expected:** Connection banner does not appear (or appears briefly then clears). Output stream resumes. Session chat is current. No user interaction required.
