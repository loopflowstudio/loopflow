# Review: Optimistic UX — Items 04, 05, 06

## What was implemented

Items 04 (optimistic create/delete), 05 (responsive actions), and 06 (RunStore) from the UX roadmap, shipped as one branch. This completes the roadmap.

**Optimistic create/delete** (item 04):
- `insertPending`, `replacePending`, `removePending` on WaveStore for create/clone lifecycle
- `applyDelete` + `commitMutation`/`rollback` for delete
- `setAll` refactored to preserve pending inserts, deletes, and edits uniformly
- `createWave` simplified — `Wave(id:name:repo:)` with defaults
- `cloneWave` optimistically inserts a copy before the API call

**Responsive actions** (item 05):
- `runWave`, `stopWave`, `nextWave` use `applyOptimistic` for transitional status
- `landWave` uses `inFlightActions: Set<String>` to disable buttons without faking status
- `optimistic()` and `optimisticAction()` helpers extract the snapshot/commit/rollback pattern
- `scheduleRefresh()` fires a 10-second safety net for daemon crash recovery
- Local view state (`isRunning`, `isLanding`, `isNexting`) removed from StepRunner and WaveDetailPanel

**RunStore** (item 06):
- `RunStore` with `setRuns`, `runs(for:)`, `clear` — flat dictionary keyed by wave ID, capped at 50 runs
- RepoState owns RunStore, exposes `loadRuns(for:)` (fire-and-forget background fetch)
- Event handler refreshes runs on started/stopped/updated events
- WaveDetailPanel reads from RunStore — no local `@State` for runs, no loading spinner after first load
- OutputBuffer cleared on wave delete via weak reference in RepoState
- `collapsePRs` and `absorbIntoPR` moved to RepoState (all service calls route through one place)

## Key choices

1. **Two mechanisms for actions, not one.** Run/stop/next use optimistic status (instant feedback). Land uses `inFlightActions` (button disables, no status lie). Landing can genuinely fail; showing fake idle then reverting is worse than a spinner.

2. **`optimistic` and `optimisticAction` helpers.** Extract the repeated snapshot → API → commit/rollback pattern. `optimisticAction` adds `scheduleRefresh` for actions where real state arrives via WebSocket. `renameWave` and `updateWave` also use `optimistic`.

3. **Stale-while-revalidate for runs.** RunStore serves cached data instantly; `loadRuns` fires a background fetch that replaces the cache when it completes. First load still shows empty → populated, but every subsequent tab switch is instant.

4. **No optimistic mutations for runs.** Runs are read-only from the user's perspective. All updates come from API fetches triggered by events or navigation. No pendingMutations or rollback needed.

5. **Pending mutations in `setAll`.** Iterate `pendingMutations` first (preserving existing waves with in-flight edits, keeping deleted waves absent), then process incoming server waves. Handles all three pending states (edit, insert, delete) uniformly.

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
    → RunStore refreshed via loadRuns()

Runs tab opened
  → WaveDetailPanel.onAppear → repoState.loadRuns(for:)
  → Cached runs render instantly (or empty on first visit)
  → Background fetch replaces cache → UI auto-updates
```

## Risks and bottlenecks

- **`scheduleRefresh` tasks are fire-and-forget.** Multiple actions queue multiple refreshes. Each is a single-wave GET, so cost is low, but they aren't cancelled if a WebSocket event arrives first. Harmless but redundant.
- **`inFlightActions` is shared across land and next button disabling.** Currently only `landWave` writes to `inFlightActions`, so Next won't self-disable — it relies on optimistic status change to `.idle` which re-renders the panel away from the ops bar.

## What's not included

- Error toast/banner UI (already exists via `showingActionError`)
- Server-side run event enrichment (not needed for local daemon latency)
- Run-level optimistic mutations (no user writes to runs)
- Persisting runs across app restarts (in-memory only per wave principles)
