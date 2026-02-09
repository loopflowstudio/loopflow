---
status: todo
seq: 5
---

# Responsive actions

## Problem

Action methods (`runWave`, `stopWave`, `landWave`, `nextWave`) await the server before the UI changes. The user clicks Run and nothing happens for 100-500ms while the daemon spawns an agent. Stop, land, and next have the same delay. The UI should respond instantly.

This is different from optimistic data mutations (item 02): those set the final correct value. Actions trigger side effects whose outcome isn't known. We set a transitional state, then let WebSocket events deliver the real state.

## Approach

Use `applyOptimistic` for run/stop/next. These have predictable transitional states:

| Action | Optimistic state | Revert on error |
|--------|-----------------|-----------------|
| `run` | `.running` | Restore previous status |
| `stop` | `.idle` | Restore previous status |
| `next` | `.idle` | Restore previous status |
| `land` | No status change | N/A |

Land is special: it merges a PR and can genuinely fail. Don't fake the status. Instead, track it as an in-flight action so buttons disable.

### RepoState changes

```swift
func runWave(wave: WaveViewModel, ...) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.status = .running }
    do {
        try await waveService.run(wave.id, overrides: overrides)
        waveStore.commitMutation(wave.id)
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}

func stopWave(_ wave: WaveViewModel) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.status = .idle }
    do {
        try await waveService.stop(wave.id)
        waveStore.commitMutation(wave.id)
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}

func nextWave(_ wave: WaveViewModel) async throws {
    let snapshot = waveStore.applyOptimistic(wave.id) { $0.status = .idle }
    do {
        _ = try await waveService.nextWave(wave.id)
        waveStore.commitMutation(wave.id)
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}
```

For `land`, add an `inFlightActions: Set<String>` to RepoState that tracks wave IDs with pending land operations. No status change; just button disabling.

```swift
private(set) var inFlightActions: Set<String> = []

func isActionInFlight(_ waveId: String) -> Bool {
    inFlightActions.contains(waveId)
}

func landWave(_ wave: WaveViewModel) async throws {
    inFlightActions.insert(wave.id)
    do {
        try await waveService.landWave(wave.id)
        inFlightActions.remove(wave.id)
    } catch {
        inFlightActions.remove(wave.id)
        throw error
    }
}
```

### View changes

**StepRunner**: Remove local `isRunning` state. The run button already checks `isWaveActive` (which reads `wave.status`). After the optimistic update, `wave.status == .running` is true immediately, so the button disables and shows a spinner without any local state.

**WaveDetailPanel**: Remove `isLanding` and `isNexting` local state. Replace with `repoState.isActionInFlight(wave.id)`:

```swift
Button { landWave() } label: { ... }
    .disabled(repoState.isActionInFlight(wave.id))
```

The Land button shows a `ProgressView` when `repoState.isActionInFlight(wave.id)` is true. The Stop button disappears naturally because `wave.status` flips to `.idle` immediately on stop.

**WaveDetailPanel stop**: Currently the stop button only shows when `wave.status == .running || .waiting`. After optimistic stop sets `.idle`, the button vanishes instantly. No local state needed.

### Timeout safety net

If the WebSocket event never arrives (daemon crash), the optimistic state persists. Add a timeout: after `commitMutation`, schedule a 10-second delayed refresh for that specific wave. If a WebSocket event arrives first, it overwrites the optimistic state (desired behavior) and the refresh is redundant but harmless.

```swift
private func scheduleRefresh(for waveId: String, delay: TimeInterval = 10) {
    Task {
        try? await Task.sleep(for: .seconds(delay))
        guard waveStore.wave(for: waveId) != nil else { return }
        if let wave = try? await waveService.getWave(waveId) {
            waveStore.set(WaveViewModel(api: wave))
        }
    }
}
```

Called after `commitMutation` in `runWave`, `stopWave`, `nextWave`. Not needed for `landWave` since we don't set optimistic status.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `inFlightActions` for all four actions | Simpler: one mechanism for everything. Buttons disable, no status change | User doesn't see immediate status feedback for run/stop/next. Feels less responsive. |
| Optimistic status for land too | Consistent pattern | Landing genuinely fails (merge conflicts). Showing `.idle` then reverting to `.running` is confusing. Better to keep status honest. |
| No timeout safety net | Simpler | A stuck transitional state with no recovery is worse than a redundant refresh |

## Key decisions

1. **Use existing `applyOptimistic`/`commitMutation`/`rollback` for run/stop/next.** This follows the wave README principle: "Optimistic for data, responsive for actions." The transitional state uses the same mechanism but sets a predicted-not-known state.

2. **Land gets `inFlightActions`, not optimistic status.** The README explicitly calls this out: "Landing merges a PR -- this can genuinely fail." Disabling the button + showing a spinner is the right UX.

3. **Remove local view state (`isRunning`, `isLanding`, `isNexting`).** The store is the single source of truth. Local state duplicates and can drift. After optimistic updates, the store drives button states directly.

4. **10-second timeout refresh.** Follows the constraint: "Transitional states must not persist." A single-wave refresh is cheap insurance against daemon crashes.

## Scope

In scope:
- Optimistic status for `runWave`, `stopWave`, `nextWave`
- `inFlightActions` for `landWave` button disabling
- Remove local `isRunning`/`isLanding`/`isNexting` state from views
- Timeout safety net for stale optimistic state
- Tests for optimistic action patterns

Out of scope:
- Error toast/banner UI (already exists via `showingActionError`)
- Changes to `handleWaveEvent` (already works correctly -- overwrites optimistic state)
- Output buffer changes
- RunStore (item 06)

## Done when

1. `swift test --package-path swift` passes
2. `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` passes
3. Clicking Run immediately changes status indicator (no delay)
4. Clicking Stop immediately shows idle status
5. Clicking Land disables button and shows spinner
6. If any action fails, UI reverts cleanly
