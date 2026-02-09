---
status: todo
seq: 6
---

# RunStore: cached wave runs

Same store pattern for wave runs. Runs tab loads from cache; fetches in background.

---

## Current

The Runs tab fetches runs on demand from the API every time it's displayed. `WaveRunsTab` calls `LocalWaveService.listRuns(waveId:)` directly, with no caching.

`OutputBuffer` manages streaming output per wave, but it's independent from runs — no shared lifecycle.

## Build

**New file: `Concerto/State/RunStore.swift`**

```swift
@MainActor
@Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]] = [:]  // keyed by wave ID

    func setRuns(for waveId: String, _ runs: [WaveRun]) { ... }
    func appendRun(for waveId: String, _ run: WaveRun) { ... }
    func runs(for waveId: String) -> [WaveRun] { ... }
    func clear(for waveId: String) { ... }
}
```

**Stale-while-revalidate pattern:**

When the Runs tab opens:
1. Return cached runs immediately (may be empty on first load)
2. Fire background fetch
3. Merge response into RunStore
4. UI updates automatically via `@Observable`

```swift
// In whatever manages the runs tab
func loadRuns(for waveId: String) async {
    // Cached data renders immediately (empty array on first load)
    let fresh = try? await waveService.listRuns(waveId: waveId)
    if let fresh {
        runStore.setRuns(for: waveId, fresh)
    }
}
```

**Event integration:**

WebSocket `agent_started` and `agent_ended` events can update RunStore:
- `agent_started` → append a new in-progress run
- `agent_ended` → update the run's status and endedAt

This means the Runs tab updates live without polling.

**OutputBuffer integration:**

OutputBuffer already works well. The only change: when a wave is deleted from WaveStore, clear its output from OutputBuffer. Add a listener or have `WaveStore.remove()` notify OutputBuffer.

## Constraints

- **Don't over-cache.** Runs are append-only and cheap to fetch. The goal is avoiding a loading spinner on tab switch, not reducing API calls to zero.
- **Memory bound.** Cap cached runs per wave (50 is the current API limit — keep that).
- **RunStore is read-heavy.** Mutations are rare (only from events). Optimize for fast reads.

## Done when

1. Switching to the Runs tab shows cached data instantly (no spinner after first load)
2. New runs appear in the tab without manual refresh
3. Deleting a wave clears its runs from the store
4. Tests pass
