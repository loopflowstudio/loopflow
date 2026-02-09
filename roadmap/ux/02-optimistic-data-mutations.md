---
status: todo
seq: 2
---

# Optimistic data mutations

Rename and config updates apply to WaveStore immediately. API call fires in background. Rollback on error.

---

## Current

`renameWave` and `updateWave` in RepoState both follow the same pattern:

```swift
func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
    _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
    await refreshWaves()  // re-fetches entire wave list
}
```

Two round-trips (PATCH + GET /waves) before the user sees the name change. On a local daemon this is ~50-100ms, but it's perceptible — especially in the sidebar where the name snaps back to old briefly.

## Build

**Add to WaveStore:**

```swift
func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
    guard var wave = waves[id] else { return nil }
    let snapshot = wave
    mutation(&wave)
    set(wave)
    return snapshot  // caller keeps for rollback
}

func rollback(_ snapshot: WaveViewModel) {
    set(snapshot)
}
```

**Change RepoState mutation methods:**

```swift
func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.name = newName }

    do {
        _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
        // No refreshWaves() — event subscription or next sync will reconcile
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}
```

Same pattern for `updateWave` (area, direction, flow, stimulus, status).

**Remove `refreshWaves()` calls** from `renameWave` and `updateWave`. The WebSocket `wave.updated` event will deliver the server-confirmed state.

## Constraints

- **Rollback must restore the exact pre-mutation state**, not a stale copy. The snapshot is taken at mutation time.
- **Error handling must surface to UI.** If the PATCH fails, the user should see the value revert and get an error indication (existing `errorMessage` on RepoState, or a toast — implementer's choice).
- **Don't remove `refreshWaves()` globally yet** — only from these two methods. Other callers (run, stop, land) still need it until items 03-05.

## Done when

1. Renaming a wave in the detail panel header updates the sidebar instantly (no flicker, no wait)
2. Changing flow/area/direction/stimulus applies instantly
3. If the daemon is stopped mid-rename, the name reverts and an error appears
4. Tests pass
