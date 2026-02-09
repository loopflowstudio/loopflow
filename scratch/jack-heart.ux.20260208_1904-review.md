# WaveStore: design review

## What was implemented

Extracted all wave state from `RepoState` into a dedicated `WaveStore` class with dictionary-keyed O(1) lookups. RepoState becomes a thin orchestrator that delegates wave storage, grouping, and status tracking to `WaveStore`.

**Files changed:**
- `Concerto/State/WaveStore.swift` — new, 112 lines. Dictionary storage, derived groups, status change detection.
- `Concerto/State/RepoState.swift` — net -155 lines. Delegates to `waveStore`, selection is ID-based with computed getter/setter.
- `LoopflowCore/Models/WaveViewModel.swift` — `pendingPR` computed property moved here from view layer.
- `Concerto/Views/WaveRow.swift` — removed `pendingPR` parameter, reads from `wave.pendingPR` directly.
- `Concerto/Views/WaveSidebar.swift` — removed `pendingPR(for:)` method, type updated from `RepoState.WaveGroups` to `WaveGroups`.
- `Concerto/Views/InteractiveSessionView.swift` — O(1) lookup via `waveStore.wave(for:)` instead of array scan.
- `ConcertoTests/WaveRowTests.swift` — removed `pendingPR` parameter from test helper.

## Key choices

1. **Dictionary, not array.** `waves: [String: WaveViewModel]` gives O(1) for set/remove/lookup. The `ordered` array and `groups` struct are derived in `recompute()` on every mutation. With ~50 waves max, recomputation is negligible.

2. **Computed `selectedWave` on RepoState.** Selection is UI coordination, not data ownership. `selectedWaveId: String?` is stored; `selectedWave` is computed via `waveStore.wave(for:)`. This eliminates the old pattern of patching `selectedWave` on every upsert — the derived lookup always reflects current store state.

3. **Forwarding properties.** `repoState.waves` and `repoState.waveGroups` are computed properties that delegate to the store. Views don't need to change how they access wave data.

4. **Closure-based status notifications.** `WaveStore.onStatusChange` fires on status transitions; RepoState wires this to `NotificationService` in `init()`. Clean separation — store detects, orchestrator decides.

5. **`pendingPR` moved to WaveViewModel.** Previously duplicated as a method in both `WaveSidebar` and `RepoState`. Now a single computed property on the model. `WaveRow` reads it directly instead of receiving it as a parameter.

## How it fits together

```
Views → RepoState (forwarding properties) → WaveStore (source of truth)
  │                  │                              │
  │                  ├── selectedWaveId (stored)     ├── waves: [String: WaveViewModel]
  │                  ├── selectedWave (computed)     ├── ordered: [WaveViewModel]
  │                  └── onStatusChange → Notifs     └── groups: WaveGroups
  │
  └── InteractiveSessionView → waveStore.wave(for:) directly (O(1) lookup)
```

RepoState owns services (`LocalWaveService`, `LocalEventService`) and orchestrates flows (create, run, stop, delete, rename). All wave mutations go through `waveStore.set()` / `waveStore.setAll()` / `waveStore.remove()`.

## Risks and bottlenecks

- **`recompute()` on every dictionary mutation.** `didSet` on `waves` triggers full group recomputation. For ~50 waves this is microseconds. At 1000+ waves it could matter — but that's not a realistic scenario for a local daemon app.
- **`setAll` calls `detectStatusChange` per-wave then overwrites `previousStatuses`.** The intermediate writes to `previousStatuses` inside the loop are redundant (overwritten by the bulk assignment). Not a bug — callbacks fire correctly from the old state — but the double-write is slightly wasteful. Not worth changing.

## What's not included

- Optimistic mutations (item 02) — `applyOptimistic`/`rollback` methods not yet added to WaveStore.
- Event-driven sync (item 03) — `handleWaveEvent` still does a GET per event.
- RunStore (item 06) — separate store for wave runs.
- No new UI behavior. All views behave identically to before.
