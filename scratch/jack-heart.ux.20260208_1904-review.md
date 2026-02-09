# WaveStore + UI polish: design review

## What was implemented

Extracted all wave state from `RepoState` into a dedicated `WaveStore` class with dictionary-keyed O(1) lookups. RepoState becomes a thin orchestrator that delegates wave storage, grouping, and status tracking to `WaveStore`. Alongside that, stimulus selection moved from a modal sheet into the StepRunner inline, and sidebar code was cleaned up.

**Files changed:**
- `Concerto/State/WaveStore.swift` — new. Dictionary storage, derived groups, status change detection.
- `Concerto/State/RepoState.swift` — net reduction. Delegates to `waveStore`, selection is ID-based with computed getter/setter.
- `LoopflowCore/Models/WaveViewModel.swift` — `pendingPR` computed property moved here from view layer.
- `Concerto/Views/WaveRow.swift` — reads `wave.pendingPR` directly; stimulus display for loop/watch/cron uses computed `stimulusLabel`.
- `Concerto/Views/WaveSidebar.swift` — `selectWave()` helper, direct `waveStore.wave(for:)` lookups for keyboard/notification handling.
- `Concerto/Views/StepRunner.swift` — inline stimulus picker (pill buttons), run button adapts label/icon to stimulus type, disables when wave is active.
- `Concerto/Views/NextActionsBar.swift` — removed StimulusPicker sheet and "Set Stimulus" button (replaced by StepRunner inline).
- `Concerto/Views/InteractiveSessionView.swift` — O(1) lookup via `waveStore.wave(for:)` instead of array scan.
- `ConcertoTests/WaveRowTests.swift` — accessibility identifier changed from `wave-cron` to `wave-stimulus`.

## Key choices

1. **Dictionary, not array.** `waves: [String: WaveViewModel]` gives O(1) for set/remove/lookup. The `ordered` array and `groups` struct are derived in `recompute()` on every mutation. With ~50 waves max, recomputation is negligible.

2. **Computed `selectedWave` on RepoState.** Selection is UI coordination, not data ownership. `selectedWaveId: String?` is stored; `selectedWave` is computed via `waveStore.wave(for:)`. Eliminates the old pattern of patching `selectedWave` on every upsert.

3. **Forwarding properties.** `repoState.waves` and `repoState.waveGroups` are computed properties that delegate to the store. Views don't need to change how they access wave data.

4. **Closure-based status notifications.** `WaveStore.onStatusChange` fires on status transitions; RepoState wires this to `NotificationService` in `init()`. Store detects, orchestrator decides.

5. **`pendingPR` moved to WaveViewModel.** Previously duplicated in both `WaveSidebar` and `RepoState`. Now a single computed property on the model.

6. **Stimulus picker moved inline.** The modal `StimulusPicker` sheet in NextActionsBar was replaced by pill buttons in StepRunner. The run button label and icon adapt to the selected stimulus. The wave can't be re-run while active (button disabled).

7. **`recompute()` uses `nonFailedWithoutPR` intermediate.** Filters `!failed && pendingPR == nil` once, then derives recentActivity, active, and idle from that subset. Removes duplicated conditions.

8. **`setAll` status tracking fix.** Status change callbacks now fire from the old `previousStatuses` state consistently, then both `waves` and `previousStatuses` are overwritten in one shot. The previous version had redundant intermediate writes.

## How it fits together

```
Views → RepoState (forwarding properties) → WaveStore (source of truth)
  │                  │                              │
  │                  ├── selectedWaveId (stored)     ├── waves: [String: WaveViewModel]
  │                  ├── selectedWave (computed)     ├── ordered: [WaveViewModel]
  │                  └── onStatusChange → Notifs     └── groups: WaveGroups
  │
  ├── StepRunner → stimulus picker inline, run with selected stimulus
  └── InteractiveSessionView → waveStore.wave(for:) directly (O(1) lookup)
```

RepoState owns services (`LocalWaveService`, `LocalEventService`) and orchestrates flows (create, run, stop, delete, rename). All wave mutations go through `waveStore.set()` / `waveStore.setAll()` / `waveStore.remove()`.

## Risks and bottlenecks

- **`recompute()` on every dictionary mutation.** `didSet` on `waves` triggers full group recomputation. For ~50 waves this is microseconds. At 1000+ waves it could matter — but that's not a realistic scenario for a local daemon app.
- **`set()` fires callback before updating `waves`.** The status change callback receives the wave object directly so this works, but a callback that reads `waveStore.waves` would see stale data. Current callers (NotificationService) don't do this.

## What's not included

- Optimistic mutations (item 02) — `applyOptimistic`/`rollback` methods not yet added to WaveStore.
- Event-driven sync (item 03) — `handleWaveEvent` still does a GET per event.
- RunStore (item 06) — separate store for wave runs.
- No new user-facing features. Stimulus picker is the same UI in a different location (inline vs modal).
