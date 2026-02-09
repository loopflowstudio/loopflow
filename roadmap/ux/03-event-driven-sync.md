---
status: todo
seq: 3
---

# Event-driven sync

WebSocket events merge directly into WaveStore. Eliminate `refreshWaves()`.

---

## Current

`handleWaveEvent` in RepoState does a GET per event:

```swift
private func handleWaveEvent(_ event: WaveEvent) async {
    switch event.type {
    case .created, .updated, .started, .stopped, .waiting:
        if let wave = try? await waveService.getWave(event.waveId) {
            upsertWave(WaveViewModel(api: wave))
        }
    case .deleted:
        waves.removeAll { $0.id == event.waveId }
    }
}
```

Every event triggers a GET request. Five events = five round-trips. Combined with mutations calling `refreshWaves()`, a single user action can trigger 2-4 HTTP requests.

The `connected` event already sets waves from the event payload directly (line 294) — this is the right pattern.

## Build

**Option A: Enrich WebSocket events (preferred)**

If the daemon can include the full Wave payload in `wave.updated`/`wave.created` events, the client merges directly:

```swift
func handleWaveEvent(_ event: WaveEvent) {
    switch event.type {
    case .created, .updated, .started, .stopped, .waiting:
        if let wave = event.wave {
            waveStore.set(WaveViewModel(api: wave))
        }
    case .deleted:
        waveStore.remove(event.waveId)
    }
}
```

This requires a daemon-side change: include the `wave` field in event payloads. Check if the daemon already sends this data — if so, just parse it.

**Option B: Debounced refresh fallback**

If enriching events is out of scope, replace per-event GETs with a single debounced refresh:

```swift
private var refreshTask: Task<Void, Never>?

func handleWaveEvent(_ event: WaveEvent) {
    if event.type == .deleted {
        waveStore.remove(event.waveId)
        return
    }
    // Debounce: wait 100ms for burst events, then refresh once
    refreshTask?.cancel()
    refreshTask = Task {
        try? await Task.sleep(for: .milliseconds(100))
        guard !Task.isCancelled else { return }
        await refreshWaves()
    }
}
```

**Remove `refreshWaves()` from all remaining mutation methods.** After this item, the only callers of `refreshWaves()` should be:
- Initial connection (via `connected` event — already works)
- Manual refresh (if we keep a refresh button)

**Delete `refreshWaves()` or reduce it to a recovery mechanism** (called on reconnect, not on every action).

## Constraints

- **Option A requires checking the daemon's event format.** Read `LocalEventService.swift` and the daemon's WebSocket handler to see what fields are available.
- **Option B is a safe fallback** that still eliminates per-event GETs and reduces network from N requests to 1 debounced request per event burst.
- **The `connected` event must continue to work as-is** — it already does a full sync on connect.
- **Optimistic mutations from item 02 must not be overwritten by stale event data.** If a rename applies optimistically and then a `wave.updated` event arrives with the old name (because the PATCH hasn't landed yet), the event should not revert the optimistic change. Solution: add a `pendingMutations` set to WaveStore — skip merging events for waves with in-flight mutations.

## Done when

1. No `getWave()` calls triggered by WebSocket events (Option A) or at most one debounced `refreshWaves()` per event burst (Option B)
2. `refreshWaves()` is removed from all mutation paths
3. Wave status changes from daemon (run completes, PR created) still update UI promptly
4. Optimistic renames are not reverted by stale events
5. Tests pass
