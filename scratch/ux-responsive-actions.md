---
status: todo
seq: 5
---

# Responsive actions

Run/stop/land/next update WaveStore immediately with transitional states. Real state arrives via events.

---

## Current

Action methods wait for the server, then refresh:

```swift
func runWave(wave: WaveViewModel, ...) async throws {
    try await waveService.run(wave.id, overrides: overrides)  // wait
    await refreshWaves()                                       // wait again
}
```

The user clicks "Run" and nothing happens for 100-500ms (daemon spawns agent, creates worktree, etc). Same for stop, land, next.

## Build

**The distinction from item 02:** Data mutations (rename) are optimistic because the new value is known and correct. Actions trigger side effects whose outcome isn't known. So instead of setting the final state, set a transitional state.

```swift
func runWave(wave: WaveViewModel, ...) async throws {
    // Immediate: show "starting" state
    waveStore.applyOptimistic(wave.id) { $0.status = .running }
    // Button disables, status indicator changes — user sees response

    do {
        try await waveService.run(wave.id, overrides: overrides)
        // Server confirmed. Real status arrives via WebSocket event.
        // No refreshWaves() needed.
    } catch {
        // Revert to previous status
        waveStore.applyOptimistic(wave.id) { $0.status = wave.status }
        throw error
    }
}
```

Similarly:

| Action | Optimistic state | Rationale |
|--------|-----------------|-----------|
| `run` | `.running` | Agent is about to start |
| `stop` | `.idle` | Agent is about to stop |
| `land` | Keep current status, disable button | Landing is a git operation that can fail |
| `next` | `.idle` with incremented iteration | Next iteration is about to begin |

**Land is special.** Landing merges a PR — this can genuinely fail (merge conflicts, CI). Don't fake the outcome. Instead:
- Disable the "Land" button immediately
- Show a "Landing..." indicator
- On success, the wave disappears (or status changes) via WebSocket event
- On failure, re-enable the button and show error

**View changes:**

Action buttons should disable while their action is in-flight. Add an `inFlightActions: Set<String>` to WaveStore or RepoState that tracks which wave IDs have pending actions. Buttons check this before rendering as enabled.

```swift
Button("Land") { ... }
    .disabled(repoState.isActionInFlight(wave.id))
```

## Constraints

- **Don't set `.running` if the run call will obviously fail** (e.g., no flow configured). Let the error propagate naturally.
- **Transitional states must not persist.** If the WebSocket event never arrives (daemon crash), the UI could show a stale transitional state. Add a timeout: if no confirming event arrives within 10s, refresh that wave explicitly.
- **Existing `handleWaveEvent` (from item 03) delivers the real state**, which overwrites the transitional state. This is the desired behavior.

## Done when

1. Clicking "Run" immediately changes the status indicator (no 100ms+ delay)
2. Clicking "Stop" immediately shows idle status
3. Clicking "Land" disables the button and shows a loading state
4. If any action fails, UI reverts cleanly with an error message
5. Tests pass
