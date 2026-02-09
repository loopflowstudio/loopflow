---
status: todo
seq: 1
---

# WaveStore: extract wave state

Single `@Observable` store that owns all wave data. RepoState delegates to it instead of managing waves directly.

---

## Current

`RepoState` owns `var waves: [WaveViewModel]` and manages grouping, selection, upserts, and status tracking. Works, but mutations must go through RepoState, and the array-based storage means lookups are O(n).

Key code in `RepoState.swift`:
- `waves: [WaveViewModel]` (line 69) — array with `didSet` that rebuilds groups
- `upsertWave()` (line 336) — linear scan to find/replace
- `previousWaveStatuses` (line 90) — parallel dictionary for change detection
- `buildWaveGroups()` (line 111) — recomputes on every `waves` change

## Build

**New file: `Concerto/State/WaveStore.swift`**

```swift
@MainActor
@Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel] = [:]

    // Derived (recomputed on change)
    private(set) var ordered: [WaveViewModel] = []
    private(set) var groups: WaveGroups = .empty

    func set(_ wave: WaveViewModel) { ... }
    func setAll(_ waves: [WaveViewModel]) { ... }
    func remove(_ id: String) { ... }
    func wave(for id: String) -> WaveViewModel? { ... }
}
```

Dictionary keyed by wave ID. `ordered` and `groups` recompute when `waves` changes (same `didSet` pattern RepoState uses today, but over a dictionary).

**Changes to `RepoState.swift`:**

- Replace `var waves: [WaveViewModel]` with `let waveStore = WaveStore()`
- Replace `selectedWave: WaveViewModel?` with `selectedWaveId: String?` (derive selected wave from store)
- Move `buildWaveGroups()` into WaveStore
- Move `upsertWave()` into WaveStore
- Move `previousWaveStatuses` + `handleWaveStatusChange()` into WaveStore
- All existing call sites (`refreshWaves`, `handleWaveEvent`, mutations) call through `waveStore`

**Changes to views:**

Views currently access `repoState.waves` and `repoState.waveGroups`. Update to `repoState.waveStore.groups` etc. Alternatively, add computed properties on RepoState that forward to the store (less churn, same result).

## Constraints

- No behavior change. Before and after should be indistinguishable to the user.
- WaveStore must be `@MainActor` (same thread model as RepoState).
- Dictionary storage means group computation must sort — use the same ordering logic from `buildWaveGroups`.
- `selectedWave` becomes derived: `waveStore.wave(for: selectedWaveId)`. This means selection survives wave updates without the manual `if selectedWave?.id == wave.id` patching.

## Done when

1. `WaveStore.swift` exists with dictionary storage + group computation
2. `RepoState` delegates all wave state to `WaveStore`
3. All existing views compile and behave identically
4. `swift test --package-path swift` passes
5. `xcodebuild test -scheme Concerto -destination 'platform=macOS'` passes
