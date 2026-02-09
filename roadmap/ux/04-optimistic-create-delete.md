---
status: todo
seq: 4
---

# Optimistic create/delete

Generalize the existing `createWave` optimistic pattern. Add delete and clone.

---

## Current

`createWave` already does optimistic insertion — this is the pattern to follow:

```swift
func createWave(name: String) async throws {
    let pendingId = "pending-\(UUID().uuidString)"
    let pending = WaveViewModel(api: Wave(id: pendingId, ...))
    waves.insert(pending, at: 0)
    selectedWave = pending

    do {
        let wave = try await waveService.createWave(name: waveName, repo: repo)
        // Replace pending with real wave
        if let index = waves.firstIndex(where: { $0.id == pendingId }) {
            waves[index] = viewModel
        }
    } catch {
        waves.removeAll { $0.id == pendingId }
        throw error
    }
}
```

`deleteWave` and `cloneWave` are server-first:

```swift
func deleteWave(_ wave: WaveViewModel) async throws {
    try await waveService.deleteWave(wave.id)  // wait for server
    waves.removeAll { $0.id == wave.id }       // then remove locally
}
```

## Build

**Move create to WaveStore:**

```swift
// WaveStore
func insertPending(name: String, repo: String) -> WaveViewModel { ... }
func replacePending(_ pendingId: String, with wave: WaveViewModel) { ... }
func removePending(_ pendingId: String) { ... }
```

RepoState's `createWave` becomes a thin orchestrator calling these methods.

**Make delete optimistic:**

```swift
func deleteWave(_ wave: WaveViewModel) async throws {
    let snapshot = waveStore.remove(wave.id)  // remove immediately
    if selectedWaveId == wave.id { selectedWaveId = nil }

    do {
        try await waveService.deleteWave(wave.id)
    } catch {
        if let snapshot { waveStore.set(snapshot) }  // restore on failure
        throw error
    }
}
```

**Make clone optimistic:**

```swift
func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
    let pendingId = "pending-\(UUID().uuidString)"
    var pending = wave
    pending.api.id = pendingId  // needs a mutable copy
    pending.api.name = "\(wave.name) (copy)"
    waveStore.set(pending)
    selectedWaveId = pendingId

    do {
        let cloned = try await waveService.cloneWave(wave.id, name: nil)
        let viewModel = WaveViewModel(api: cloned)
        waveStore.replacePending(pendingId, with: viewModel)
        selectedWaveId = viewModel.id
        return viewModel
    } catch {
        waveStore.remove(pendingId)
        throw error
    }
}
```

## Constraints

- **Pending waves need visual treatment.** A wave with a `pending-` ID prefix should render normally but might show a subtle loading indicator. Or not — the server response is fast enough that the pending state is barely visible. Implementer's choice.
- **Wave.id is `let` not `var`.** To create a mutable pending copy for clone, either make a new Wave with a pending ID, or add a factory method. Don't make `id` mutable.
- **Event reconciliation (from item 03) must handle pending IDs.** A `wave.created` event from the server won't match the pending ID. The `replacePending` method handles this explicitly.

## Done when

1. Creating a wave shows it in the sidebar instantly (already works — verify it still does after WaveStore migration)
2. Deleting a wave removes it from the sidebar instantly; if delete fails, it reappears
3. Cloning a wave shows the copy instantly
4. No visible delay for any of these operations
5. Tests pass
