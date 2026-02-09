---
status: in-progress
seq: 2
---

# Optimistic data mutations

Rename and config updates apply to WaveStore immediately. API call fires in background. Rollback on error.

## Problem

`renameWave` and `updateWave` both wait for PATCH + `refreshWaves()` (GET /waves) before the UI reflects the change. That's two round-trips — 50-100ms on a local daemon. The sidebar name snaps back to old briefly, typeahead values reset, and config changes feel sluggish.

Users already know what they typed. The UI should reflect it immediately.

## Approach

Add snapshot-and-rollback primitives to WaveStore. RepoState mutations apply the change locally first, fire the API call, and roll back only on failure. No `refreshWaves()` — the WebSocket `wave.updated` event reconciles server state.

Two new methods on WaveStore:

```swift
func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
    guard var wave = waves[id] else { return nil }
    let snapshot = wave          // copy before mutation
    mutation(&wave)
    set(wave)                    // immediate UI update via didSet → recompute
    return snapshot              // caller keeps for rollback
}

func rollback(_ snapshot: WaveViewModel) {
    set(snapshot)                // restores exact pre-mutation state
}
```

RepoState becomes a thin orchestrator:

```swift
func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.name = newName }
    do {
        _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}

func updateWave(_ wave: WaveViewModel, area: [String]? = nil, direction: [String]? = nil,
                flow: String? = nil, stimulus: Stimulus? = nil, status: WaveStatus? = nil) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { w in
        if let area { w.area = area }
        if let direction { w.direction = direction }
        if let flow { w.flow = flow }
        if let stimulus { w.stimulus = stimulus }
        if let status { w.status = status }
    }
    do {
        let config = WaveConfigUpdate(area: area, direction: direction, flow: flow,
                                      stimulus: stimulus, status: status)
        _ = try await waveService.updateWave(wave.id, config: config)
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}
```

Both methods remove `refreshWaves()`. The `wave.updated` WebSocket event (handled in `handleWaveEvent`) already calls `waveStore.set()` with server-confirmed state — that's the reconciliation path.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Debounced refresh | Still has a visible delay; adds complexity with timers | Optimistic is simpler and instant |
| Separate "pending" state per field | Track which fields are in-flight, merge carefully | Over-engineered for single-user local daemon |
| Move mutation logic into WaveStore | Store knows about API calls | Violates separation — store is data, not orchestration |
| Publisher/subscriber pattern | Views subscribe to specific field changes | SwiftUI `@Observable` already handles this; adding Combine is noise |

## Key decisions

**Snapshot is the full WaveViewModel, not individual fields.** A rename could in theory coincide with a status change from a WebSocket event. Snapshotting the whole model means rollback restores the exact pre-mutation state. This is correct because the mutation closure only touches the fields being changed — all other fields in the snapshot were current at mutation time.

**Rollback uses the same `set()` path as normal updates.** This triggers `recompute()` (groups, ordering) and `onStatusChange` notifications. No special rollback code path needed.

**No `refreshWaves()` in mutation methods.** Per wave README principle: "Events merge into store; kill `refreshWaves()`." The `handleWaveEvent` path (item 03) will deliver server truth. Until item 03 lands, the current `handleWaveEvent` does a per-event GET — that's fine, it still reconciles.

**Errors surface through the existing throw path.** `renameWave` and `updateWave` already `throw`. Callers in WaveDetailPanel catch and show `actionError`. Callers in StepRunner/WaveSidebar use `try?` — the rollback still fires (it's before the throw), so the UI reverts even if the error isn't displayed. This is the right behavior: silently reverting a failed config change is better than leaving stale optimistic state.

**`applyOptimistic` returns `Optional<WaveViewModel>`.** Returns `nil` if the wave doesn't exist (deleted between user action and mutation). Callers check `if let snapshot` before rolling back. This handles the race gracefully.

## Scope

### In scope
- `applyOptimistic` and `rollback` methods on WaveStore
- Refactor `renameWave` to apply optimistically
- Refactor `updateWave` to apply optimistically
- Remove `refreshWaves()` from both methods
- Unit tests for WaveStore optimistic/rollback
- Unit tests for RepoState mutation flow (with mock service)

### Out of scope
- Event-driven sync (item 03) — `handleWaveEvent` still does per-event GET
- Optimistic create/delete (item 04) — `createWave` already has its own pattern
- Responsive actions (item 05) — run/stop/land/next unchanged
- `refreshWaves()` removal from other callers
- Pending mutation tracking for event reconciliation (item 03 concern)

## Implementation

### 1. WaveStore: add `applyOptimistic` and `rollback` (~10 LOC)

Add after the existing `remove` method in WaveStore.swift:

```swift
// MARK: - Optimistic mutations

func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
    guard var wave = waves[id] else { return nil }
    let snapshot = wave
    mutation(&wave)
    set(wave)
    return snapshot
}

func rollback(_ snapshot: WaveViewModel) {
    set(snapshot)
}
```

### 2. RepoState: refactor `renameWave` (~10 LOC)

Replace the current implementation. Apply optimistic, fire API, rollback on error. No `refreshWaves()`.

### 3. RepoState: refactor `updateWave` (~15 LOC)

Same pattern. The mutation closure conditionally applies each non-nil field.

### 4. Tests: WaveStore optimistic behavior (~40 LOC)

New file `ConcertoTests/WaveStoreTests.swift`:

- `applyOptimistic` updates the wave and returns snapshot with old values
- `rollback` restores the snapshot exactly
- `applyOptimistic` returns nil for missing wave ID
- Groups recompute after optimistic mutation (e.g., status change moves wave between groups)

### 5. Tests: RepoState mutation flow (~60 LOC)

Need a mock `WaveServiceProtocol` to test RepoState in isolation. Either:
- Extract a protocol from `LocalWaveService` (preferred — enables all future testing)
- Or test WaveStore directly and trust the thin RepoState orchestration

Decision: extract protocol. It's ~10 LOC and unblocks testing for items 03-06 too.

New file `ConcertoTests/WaveStoreTests.swift` (or extend):

- Rename applies immediately, API called after, no refreshWaves
- Rename with API failure: wave reverts to original name
- Update with multiple fields applies all immediately
- Update with API failure: all fields revert

## Done when

1. Renaming a wave in the detail panel header updates the sidebar instantly (no flicker, no wait)
2. Changing flow/area/direction/stimulus applies instantly
3. If the daemon is stopped mid-rename, the name reverts and an error appears
4. `refreshWaves()` is not called from `renameWave` or `updateWave`
5. Tests pass: `swift test --package-path swift` and Concerto UI tests
