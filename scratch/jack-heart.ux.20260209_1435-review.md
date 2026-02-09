# Review: Responsive Actions + Optimistic Create/Delete

## What was implemented

Items 04 (optimistic create/delete) and 05 (responsive actions) from the UX roadmap, shipped as one branch.

**Optimistic create/delete** (item 04):
- `insertPending`, `replacePending`, `removePending` on WaveStore for create/clone lifecycle
- `applyDelete` + `commitMutation`/`rollback` for delete
- `setAll` refactored to preserve both pending inserts and pending deletes
- `createWave` simplified — uses `Wave(id:name:repo:)` with defaults instead of spelling out every field
- `cloneWave` now optimistically inserts a copy before the API call

**Responsive actions** (item 05):
- `runWave`, `stopWave`, `nextWave` use `applyOptimistic` to set transitional status immediately
- `landWave` uses `inFlightActions: Set<String>` to disable buttons without faking status
- `optimistic()` and `optimisticAction()` private helpers extract the snapshot/commit/rollback pattern
- `scheduleRefresh()` fires a 10-second safety net for daemon crash recovery
- Local view state (`isRunning`, `isLanding`, `isNexting`) removed from StepRunner and WaveDetailPanel

## Key choices

1. **Two mechanisms, not one.** Run/stop/next use optimistic status (user sees instant feedback). Land uses `inFlightActions` (button disables, no status lie). Landing can genuinely fail; showing fake idle then reverting is worse than a spinner.

2. **`optimistic` and `optimisticAction` helpers.** Extracted the repeated snapshot → API → commit/rollback pattern into two helpers. `optimisticAction` adds `scheduleRefresh` for actions where real state arrives via WebSocket. `renameWave` and `updateWave` also use `optimistic` — no duplicated error handling.

3. **`defer` in `landWave`.** Uses `defer { inFlightActions.remove(wave.id) }` instead of explicit cleanup in both success/error paths. Concise and correct.

4. **Pending mutations in `setAll`.** Refactored to iterate `pendingMutations` first (preserving existing waves with in-flight edits, keeping deleted waves absent), then process incoming server waves. This handles all three pending states (edit, insert, delete) uniformly.

## How it fits together

```
User clicks Run
  → RepoState.runWave()
    → WaveStore.applyOptimistic() sets .running
    → UI re-renders immediately (status indicator, button state)
    → API call to daemon
    → WaveStore.commitMutation() unblocks events
    → scheduleRefresh() queues 10s safety net
  → WebSocket event arrives with real status
    → WaveStore.set() overwrites (events now unblocked)
```

For land: `inFlightActions.insert()` → button disables → API call → `defer` removes on any exit path.

## Risks and bottlenecks

- **`scheduleRefresh` tasks are fire-and-forget.** If many actions happen quickly, multiple refreshes queue. Each is a single-wave GET, so cost is low, but they aren't cancelled if a WebSocket event arrives first. The refresh is harmless but redundant.
- **`inFlightActions` is shared across land and next button disabling.** Both Land and Next disable when any `inFlightActions` entry exists for the wave. Currently only `landWave` writes to `inFlightActions`, so Next won't self-disable — it relies on optimistic status change to `.idle` which re-renders the panel away from the ops bar. This coupling is fine for now but would need revisiting if more actions used `inFlightActions`.

## What's not included

- Error toast/banner UI (already exists via `showingActionError`)
- Changes to `handleWaveEvent` (already correct — overwrites optimistic state)
- OutputBuffer changes
- RunStore (item 06, next)
