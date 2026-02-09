# Optimistic create/delete

## Problem

`deleteWave` and `cloneWave` are server-first — the user clicks and waits 100-300ms before seeing any UI change. `createWave` already uses optimistic insertion with `pending-` IDs but doesn't leverage WaveStore's `pendingMutations` guard, so a `setAll` from reconnection could leave a stale pending wave in the store.

All three operations should feel instant.

## Approach

Four new WaveStore methods that compose with the existing `pendingMutations` guard. RepoState methods become thin orchestrators.

### WaveStore additions

```swift
// Insert a pending wave and guard it from external set/setAll
func insertPending(_ wave: WaveViewModel) {
    pendingMutations.insert(wave.id)
    _set(wave)
}

// Atomically swap a pending wave for the real server wave
func replacePending(_ pendingId: String, with wave: WaveViewModel) {
    pendingMutations.remove(pendingId)
    waves.removeValue(forKey: pendingId)
    previousStatuses.removeValue(forKey: pendingId)
    _set(wave)
}

// Remove a pending wave and clear its guard
func removePending(_ id: String) {
    pendingMutations.remove(id)
    waves.removeValue(forKey: id)
    previousStatuses.removeValue(forKey: id)
}

// Remove a wave optimistically while blocking event re-insertion
func applyDelete(_ id: String) {
    pendingMutations.insert(id)
    waves.removeValue(forKey: id)
}
```

### RepoState: createWave (hardened)

```swift
func createWave(name: String) async throws {
    guard let repo = currentRepo else { return }
    let waveName = name.isEmpty ? NameGenerator.generate() : name

    let pendingId = "pending-\(UUID().uuidString)"
    let pending = WaveViewModel(
        api: Wave(id: pendingId, name: waveName, repo: repo.path)
    )
    waveStore.insertPending(pending)
    selectedWaveId = pendingId

    do {
        let wave = try await waveService.createWave(name: waveName, repo: repo)
        waveStore.replacePending(pendingId, with: WaveViewModel(api: wave))
        selectedWaveId = wave.id
    } catch {
        waveStore.removePending(pendingId)
        if selectedWaveId == pendingId { selectedWaveId = nil }
        throw error
    }
}
```

### RepoState: deleteWave (optimistic)

```swift
func deleteWave(_ wave: WaveViewModel) async throws {
    waveStore.applyDelete(wave.id)
    if selectedWaveId == wave.id { selectedWaveId = nil }

    do {
        try await waveService.deleteWave(wave.id)
        waveStore.commitMutation(wave.id)
    } catch {
        waveStore.rollback(wave)
        throw error
    }
}
```

`applyDelete` adds the ID to `pendingMutations` and removes from `waves`. This blocks any WebSocket `wave.updated` event from re-inserting the wave during the API call. On success, `commitMutation` clears the guard. On failure, `rollback(wave)` re-inserts via `_set` and clears `pendingMutations`.

### RepoState: cloneWave (optimistic)

```swift
func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
    let pendingId = "pending-\(UUID().uuidString)"
    let pendingWave = Wave(
        id: pendingId,
        name: "\(wave.name) (copy)",
        repo: wave.api.repo,
        flow: wave.api.flow,
        direction: wave.api.direction,
        area: wave.api.area,
        stimulus: wave.api.stimulus,
        status: .idle,
        iteration: 0
    )
    let pending = WaveViewModel(api: pendingWave)
    waveStore.insertPending(pending)
    selectedWaveId = pendingId

    do {
        let cloned = try await waveService.cloneWave(wave.id, name: nil)
        let viewModel = WaveViewModel(api: cloned)
        waveStore.replacePending(pendingId, with: viewModel)
        selectedWaveId = viewModel.id
        return viewModel
    } catch {
        waveStore.removePending(pendingId)
        if selectedWaveId == pendingId { selectedWaveId = nil }
        throw error
    }
}
```

Since `Wave.id` is `let`, the pending copy is a new `Wave` struct with properties copied from the source. Server-side fields (`commits`, `diffStat`, `activeRun`) default to empty.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Use `applyOptimistic` for delete (set status to "deleting") | Shows "deleting" state briefly | Removal is simpler and faster than a status transition |
| Make `Wave.id` mutable for clone copies | Simplest code for clone | Violates immutable identity; id should be stable |
| No pending guard for delete (just remove + restore) | Simpler | Race: a WebSocket event during the API call would re-insert the wave |

## Key decisions

1. **Four new WaveStore methods** (`insertPending`, `replacePending`, `removePending`, `applyDelete`) compose with the existing `pendingMutations` guard. "Single source of truth. WaveStore is canonical."

2. **Delete uses `applyDelete` + `rollback`.** The pending guard blocks WebSocket events from re-inserting a wave during the delete API call.

3. **Clone creates a new `Wave` struct.** `Wave.id` is `let`. Only essential config fields are copied; server-side state defaults to empty.

4. **No visual pending indicator.** Server responses are <200ms for the local daemon. A loading spinner would flash distractingly.

5. **`handleWaveEvent` needs no changes.** `set()` is a no-op for waves in `pendingMutations`. `remove()` is a no-op if the wave is already gone.

## Scope

- In scope: optimistic create (harden existing), optimistic delete, optimistic clone, WaveStore methods, tests
- Out of scope: visual pending indicators, run/stop/land (item 05), undo

## Tests

In `WaveStoreTests.swift`:

1. `insertPending` adds wave and blocks `set()` for that ID
2. `replacePending` atomically swaps pending for real wave, unblocks `set()`
3. `removePending` removes wave and clears pending state
4. `applyDelete` removes wave but blocks `set()` (prevents event re-insertion)
5. `applyDelete` + `rollback` restores the wave
6. `setAll` during pending delete doesn't re-insert the wave

## Done when

1. Creating a wave shows it in the sidebar instantly (verify existing behavior preserved)
2. Deleting a wave removes it from the sidebar instantly; if delete fails, it reappears
3. Cloning a wave shows the copy instantly; replaced with real wave on server response
4. No visible delay for any of these operations
5. `swift test --package-path swift` passes
6. `xcodebuild test` passes
