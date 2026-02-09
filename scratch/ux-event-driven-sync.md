---
status: todo
seq: 3
---

# Event-driven sync

WebSocket events merge directly into WaveStore. Eliminate per-event GET requests and most `refreshWaves()` calls.

## Problem

Every WebSocket event triggers an HTTP GET. The daemon fires `wave_updated` when a wave starts, when a step changes, when a run completes, when config changes. A single "run" action produces 3-5 events, each causing a GET to `/api/waves/{id}`. Combined with mutation methods calling `refreshWaves()` (full list fetch), a single user action can generate 4-8 HTTP round-trips to a local daemon.

The data is already known — the daemon just changed it. It's one process away from the client. The HTTP requests exist because events carry only `wave_id`, not the wave itself.

## Approach

**Enrich WebSocket events with the full WaveDto payload.** The daemon already builds `WaveDto` for the `connected` event. Do the same for wave lifecycle events. The client merges the payload directly into WaveStore — zero HTTP requests per event.

On the Rust side, the WebSocket handler (`ws.rs`) receives `Event` from the broadcast channel. Instead of serializing the raw `Event`, it enriches wave events by fetching the current `WaveDto` before sending. This keeps the `Event` enum lean (no `WaveDto` in the broadcast channel) and centralizes the enrichment in the WebSocket layer, which already knows how to build DTOs.

```rust
// ws.rs — in the event select branch
maybe_event = events.next() => {
    let Some(event) = maybe_event else { break };
    if let Ok(event) = event {
        let json = match enrich_event(&event, &state.store).await {
            Some(enriched) => enriched,
            None => serde_json::to_string(&event).unwrap_or_default(),
        };
        if sender.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}
```

```rust
async fn enrich_event(event: &Event, store: &SharedStore) -> Option<String> {
    let wave_id = match event {
        Event::WaveCreated { wave_id, .. }
        | Event::WaveUpdated { wave_id, .. }
        | Event::WaveStarted { wave_id, .. }
        | Event::WaveStopped { wave_id, .. }
        | Event::WaveWaiting { wave_id, .. } => wave_id.clone(),
        _ => return None,
    };

    let event_type = match event {
        Event::WaveCreated { .. } => "wave_created",
        Event::WaveUpdated { .. } => "wave_updated",
        Event::WaveStarted { .. } => "wave_started",
        Event::WaveStopped { .. } => "wave_stopped",
        Event::WaveWaiting { .. } => "wave_waiting",
        _ => return None,
    };

    let wave = run_store(store, move |s| s.get_wave(&wave_id)).await.ok()?;
    let wave = wave?;
    let dto = build_wave_dto(store, wave, true).await.ok()?;

    // Build enriched JSON: original event fields + "wave" field
    let mut base = serde_json::to_value(event).ok()?;
    if let serde_json::Value::Object(ref mut map) = base {
        map.insert("wave".to_string(), serde_json::to_value(&dto).ok()?);
    }
    serde_json::to_string(&base).ok()
}
```

On the Swift side, `WaveEvent` gains an optional `wave: Wave?` field. `handleWaveEvent` uses it when present, falls back to GET when absent (forward compatibility).

```swift
// LocalEventService — parseEvent updates
case "wave_updated":
    guard let waveId = json["wave_id"] as? String else { return nil }
    let wave = (json["wave"] as? [String: Any]).map { LocalWaveService.parseWaveFromJSON($0) }
    return .wave(WaveEvent(
        type: .updated, waveId: waveId, waveRunId: nil,
        step: nil, name: nil, wave: wave,
        timestamp: parseTimestamp(json["timestamp"])
    ))

// RepoState
private func handleWaveEvent(_ event: WaveEvent) async {
    switch event.type {
    case .created, .updated, .started, .stopped, .waiting:
        if let wave = event.wave {
            waveStore.set(WaveViewModel(api: wave))
        } else if let wave = try? await waveService.getWave(event.waveId) {
            waveStore.set(WaveViewModel(api: wave))
        }
    case .deleted:
        waveStore.remove(event.waveId)
        if selectedWaveId == event.waveId {
            selectedWaveId = nil
        }
    }
}
```

**Remove `refreshWaves()` from all mutation and action methods.** After this change:
- `runWave` — no `refreshWaves()`, event delivers state
- `stopWave` — no `refreshWaves()`, event delivers state
- `landWave` — no `refreshWaves()`, event delivers state
- `nextWave` — no `refreshWaves()`, event delivers state
- `refreshRunsAndWaves` in WaveDetailPanel — calls `fetchRuns()` only, not `refreshWaves()`

Keep `refreshWaves()` alive for two callers:
1. **Command palette "Refresh Waves"** — manual recovery mechanism
2. **Reconnection** — via `connected` event (already works, uses `setAll`)

**Add `pendingMutations` guard to WaveStore.** When an optimistic mutation is in flight (e.g., rename), a `wave.updated` event could arrive with the pre-mutation name (the PATCH hasn't landed yet). The guard prevents stale events from reverting optimistic state.

```swift
// WaveStore
private var pendingMutations: Set<String> = []

func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
    guard var wave = waves[id] else { return nil }
    let snapshot = wave
    mutation(&wave)
    pendingMutations.insert(id)
    set(wave)
    return snapshot
}

func commitMutation(_ id: String) {
    pendingMutations.remove(id)
}

func rollback(_ snapshot: WaveViewModel) {
    pendingMutations.remove(snapshot.id)
    set(snapshot)
}

func set(_ wave: WaveViewModel) {
    // Skip if this wave has a pending optimistic mutation
    // (unless called from applyOptimistic itself)
    // ...see Key Decisions below
}
```

The tricky part: `set` is called from both `applyOptimistic` (should always apply) and from `handleWaveEvent` (should skip if pending). Rather than adding a flag parameter, split the internal path:

```swift
func set(_ wave: WaveViewModel) {
    guard !pendingMutations.contains(wave.id) else { return }
    _set(wave)
}

private func _set(_ wave: WaveViewModel) {
    let oldStatus = waves[wave.id]?.status
    waves[wave.id] = wave
    if let oldStatus, oldStatus != wave.status {
        onStatusChange?(wave, oldStatus, wave.status)
    }
}

func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
    guard var wave = waves[id] else { return nil }
    let snapshot = wave
    mutation(&wave)
    pendingMutations.insert(id)
    _set(wave)  // bypass pending check
    return snapshot
}
```

Then in RepoState, after the API call succeeds:

```swift
func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.name = newName }
    do {
        _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
        waveStore.commitMutation(wave.id)  // allow events through again
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Debounced refresh (Option B from ingested item) | Reduces N GETs to 1, but still round-trips. 100ms debounce adds visible latency. | Enriched events are strictly better — zero latency, zero HTTP calls. The debounce is a half-measure. |
| Enrich the `Event` enum with `WaveDto` | Broadcast channel carries full DTOs. All subscribers get the data. | Bloats the broadcast channel. `OutputLine` events would share a channel with large DTOs. The `Event` enum is also `Clone` — copying DTOs on every broadcast is wasteful. Enrichment at the WebSocket edge is cheaper. |
| Client-side merge from event fields | Parse `status`, `iteration` etc. from event fields without a full DTO. | Events would need to grow incrementally as new fields are added to Wave. Fragile, hard to keep in sync. Full DTO is the right unit. |
| Remove `refreshWaves()` entirely | No fallback for desync. | Need it for manual recovery and reconnection. Keeping it for those two cases costs nothing. |
| No pending mutations guard | Simpler. Accept that stale events briefly flash old data. | Visible regression from item 02. User renames wave, sees old name flash back for 50ms, then corrects. Unacceptable. |

## Key decisions

**Enrichment happens in ws.rs, not in the Event enum.** The broadcast channel stays lightweight. Only WebSocket clients pay the cost of building DTOs. If other subscribers (e.g., future webhook support) need enriched events, they can do their own enrichment.

**Fallback GET when `event.wave` is nil.** Forward compatibility. If a daemon version doesn't enrich events (version skew), the client degrades gracefully to the current per-event GET behavior. This also handles edge cases where `enrich_event` fails (wave deleted between event and enrichment).

**`pendingMutations` is a Set<String>, not a per-field tracker.** Single user, local daemon. If a wave has any pending mutation, skip all incoming events for that wave. The mutation will `commitMutation` when the API confirms, and the next event will bring the true state. Per-field tracking is overkill — the window where stale events arrive is <100ms.

**`commitMutation` happens on API success, not on event receipt.** The API call is the confirmation that the server accepted the change. Once committed, subsequent events are safe to merge. If we committed on event receipt, we'd need to match the event's data against the optimistic data — complex and fragile.

**`build_wave_dto` in the enrichment path calls `spawn_blocking` for git state.** This is the same code path as `connected` and GET handlers. It's ~5ms for a single wave. Acceptable for WebSocket events that arrive at most a few per second. The alternative (skip git state in events) would mean events lack `commits` and `diff_stat` — the UI would need a separate fetch for those.

**Per wave README principle: "Events merge into store; kill `refreshWaves()`."** This design follows that principle exactly. Events merge directly. `refreshWaves()` survives only as a recovery mechanism.

## Scope

### In scope
- Rust: enrich wave events with `WaveDto` in `ws.rs`
- Swift: parse optional `wave` field from WebSocket events
- Swift: `handleWaveEvent` uses event payload, falls back to GET
- Swift: remove `refreshWaves()` from `runWave`, `stopWave`, `landWave`, `nextWave`
- Swift: remove `refreshWaves()` from `refreshRunsAndWaves` in WaveDetailPanel
- Swift: `pendingMutations` guard in WaveStore
- Swift: `commitMutation` in RepoState mutation methods
- Tests: WaveStore pending mutations behavior
- Tests: handleWaveEvent with and without enriched payload
- Tests: Rust enrichment serialization

### Out of scope
- Optimistic create/delete (item 04) — separate concern
- Responsive actions (item 05) — transitional states
- RunStore (item 06)
- Removing `refreshWaves()` from command palette — it stays as manual recovery
- `WorktreeUpdated` event enrichment — not a wave event, no change needed
- Agent event enrichment — `agentStarted`/`agentEnded` don't affect WaveStore directly

## Implementation

### 1. Rust: `enrich_event` in ws.rs (~30 LOC)

Add `enrich_event` function. Modify the event select branch to call it before serializing. Uses existing `build_wave_dto` and `run_store` helpers.

### 2. Swift: Add `wave` to `WaveEvent` (~5 LOC)

Add `public let wave: Wave?` to `WaveEvent`. Update all constructor sites in `parseEvent` to parse the optional `wave` JSON object using existing `LocalWaveService.parseWaveFromJSON`.

### 3. Swift: Update `handleWaveEvent` (~5 LOC)

Check `event.wave` first. Fall back to `getWave()` if nil.

### 4. Swift: Remove `refreshWaves()` from mutation/action methods (~10 LOC, deletions)

Remove from: `runWave`, `stopWave`, `landWave`, `nextWave`. Remove from `refreshRunsAndWaves` in WaveDetailPanel (keep only `fetchRuns()`).

### 5. Swift: `pendingMutations` in WaveStore (~25 LOC)

Add `pendingMutations` set. Split `set` into public `set` (with guard) and private `_set` (no guard). Add `commitMutation`. Update `applyOptimistic` and `rollback` to manage the set.

### 6. Swift: `commitMutation` calls in RepoState (~4 LOC)

Add `waveStore.commitMutation(wave.id)` after successful API calls in `renameWave` and `updateWave`.

### 7. Tests (~50 LOC)

- WaveStore: `set` skips waves with pending mutations
- WaveStore: `commitMutation` allows subsequent `set` calls through
- WaveStore: `rollback` clears pending state
- Rust: `enrich_event` produces JSON with both event fields and `wave` object
- Rust: `enrich_event` returns `None` for non-wave events

## Done when

1. Wave status changes (run starts, completes, fails) update UI with zero HTTP GETs — verified by checking daemon logs for GET requests during a run
2. `refreshWaves()` is not called from any mutation or action method
3. Renaming a wave while an event arrives does not flash the old name
4. Command palette "Refresh Waves" still works as manual recovery
5. Daemon restart triggers `connected` event → full resync still works
6. `swift test --package-path swift` passes
7. `cargo test --all` passes
8. Concerto UI tests pass
