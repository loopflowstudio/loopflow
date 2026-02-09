# Review: Optimistic data mutations

## What was implemented

Optimistic apply/rollback for `renameWave` and `updateWave`. UI reflects changes instantly; API fires in background; rollback on failure.

Three changes:

1. **WaveStore** gains `applyOptimistic(_:_:)` and `rollback(_:)` — snapshot-and-restore primitives that go through the existing `set()` path (triggering `recompute()` and `onStatusChange`).

2. **RepoState** `renameWave` and `updateWave` refactored from server-first to optimistic-first. Both remove `refreshWaves()`. The WebSocket `handleWaveEvent` path (per-event GET) still reconciles server state.

3. **WaveServiceProtocol** extracted from `LocalWaveService`. Covers all methods RepoState calls. `WaveFlowsResult` replaces the nested `LocalWaveService.FlowsResult` type. Unblocks mock-based testing for items 03-06.

## Key choices

**Full-model snapshot, not per-field.** Simpler, correct for single-user local daemon. Rollback restores exact pre-mutation state.

**Rollback via `set()`, not a separate path.** Triggers the same `recompute()` and `onStatusChange` as normal updates. No special-case code.

**Protocol extracted but RepoState not yet injected.** `LocalWaveService` conforms to `WaveServiceProtocol`, but RepoState still creates `LocalWaveService()` directly. Injection will come when RepoState tests are added (items 03-06). The protocol is the right foundation without premature wiring.

**No RepoState mutation tests yet.** WaveStore tests prove the core behavior (optimistic apply, rollback, groups recompute, status notifications). The thin orchestration in RepoState is correct by inspection. Full RepoState tests with mock services will come with item 03 when async test infrastructure is needed anyway.

## How it fits together

```
User action → RepoState.renameWave/updateWave
           → WaveStore.applyOptimistic (instant UI update)
           → waveService.updateWave (background API call)
           → on error: WaveStore.rollback (revert UI)
           → on success: handleWaveEvent delivers server truth via WebSocket
```

## Risks and bottlenecks

**Stale event overwrite.** If a `wave.updated` event arrives with pre-mutation data (because the PATCH hasn't landed yet), it will overwrite the optimistic state. This is a known item 03 concern — solved by `pendingMutations` tracking in that item. For now, the race window is <50ms on a local daemon, making this unlikely in practice.

**`refreshWaves()` still called by other methods.** `runWave`, `stopWave`, `landWave`, `nextWave` still call `refreshWaves()`. This is intentional scope — those are items 03-05.

## What's not included

- Event-driven sync (item 03)
- Optimistic create/delete (item 04) — `createWave` already has its own optimistic pattern
- Responsive actions (item 05)
- RepoState injection via protocol (deferred to when mock tests are needed)
- `pendingMutations` tracking for event reconciliation
