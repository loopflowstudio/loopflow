---
status: done
seq: 1
---

# WaveStore: extract wave state

## Problem

RepoState manages 565 lines of wave lifecycle, grouping, selection, status tracking, event handling, CRUD, and service orchestration — all in one class. Every mutation goes through RepoState. Lookups are O(n) array scans. The `selectedWave` property must be manually patched on every upsert. This coupling blocks the next 5 items in the ux wave (optimistic mutations, event-driven sync, responsive actions) because they all need to mutate wave state independently of the HTTP/event lifecycle RepoState orchestrates.

Extracting a `WaveStore` gives us dictionary-keyed O(1) lookups, derived selection, and a clean API surface for the optimistic/rollback pattern coming in items 02-05.

## Approach

New file `Concerto/State/WaveStore.swift`: a `@MainActor @Observable` class that owns all wave data. RepoState becomes the orchestrator (services, events, flows) and delegates wave state entirely to `waveStore`.

### WaveStore API

```swift
@MainActor
@Observable
final class WaveStore {
    // Primary storage — dictionary keyed by wave ID
    private(set) var waves: [String: WaveViewModel] = [:] {
        didSet { recompute() }
    }

    // Derived state — recomputed on any change to waves
    private(set) var ordered: [WaveViewModel] = []
    private(set) var groups: WaveGroups = .empty

    // Status tracking for notifications (moved from RepoState)
    private var previousStatuses: [String: WaveStatus] = [:]
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    // MARK: - Mutations

    func set(_ wave: WaveViewModel) {
        detectStatusChange(wave)
        waves[wave.id] = wave
    }

    func setAll(_ newWaves: [WaveViewModel]) {
        for wave in newWaves { detectStatusChange(wave) }
        waves = Dictionary(uniqueKeysWithValues: newWaves.map { ($0.id, $0) })
        previousStatuses = Dictionary(uniqueKeysWithValues: newWaves.map { ($0.id, $0.status) })
    }

    @discardableResult
    func remove(_ id: String) -> WaveViewModel? {
        let removed = waves.removeValue(forKey: id)
        previousStatuses.removeValue(forKey: id)
        return removed
    }

    func removeAll() {
        waves = [:]
        previousStatuses = [:]
    }

    // MARK: - Queries

    func wave(for id: String) -> WaveViewModel? { waves[id] }

    var isEmpty: Bool { waves.isEmpty }
    var count: Int { waves.count }
}
```

### WaveGroups — moved out of RepoState

`WaveGroups` becomes a standalone struct (or stays nested, but gets a static `.empty`):

```swift
struct WaveGroups {
    let blocked: [WaveViewModel]
    let pr: [WaveViewModel]
    let recentActivity: [WaveViewModel]
    let active: [WaveViewModel]
    let idle: [WaveViewModel]

    var attentionCount: Int { blocked.count + pr.count }
    var allInOrder: [WaveViewModel] { blocked + pr + recentActivity + active + idle }

    static let empty = WaveGroups(blocked: [], pr: [], recentActivity: [], active: [], idle: [])
}
```

The `buildWaveGroups()` and `pendingPR(for:)` methods move into `WaveStore.recompute()`. Same logic, now operating over `waves.values` and sorting for deterministic output (dictionary has no inherent order).

### Status change detection

`previousWaveStatuses` and `handleWaveStatusChange()` move into WaveStore. WaveStore detects transitions in `detectStatusChange()` and calls `onStatusChange` closure — RepoState sets this closure to invoke `NotificationService`. This keeps notification logic out of the store while letting the store own status tracking.

```swift
private func detectStatusChange(_ wave: WaveViewModel) {
    let old = previousStatuses[wave.id]
    if old != wave.status {
        onStatusChange?(wave, old, wave.status)
    }
    previousStatuses[wave.id] = wave.status
}
```

### Selection — ID-based

RepoState replaces `selectedWave: WaveViewModel?` with:

```swift
var selectedWaveId: String?

var selectedWave: WaveViewModel? {
    get { selectedWaveId.flatMap { waveStore.wave(for: $0) } }
    set { selectedWaveId = newValue?.id }
}
```

The computed `selectedWave` property preserves the existing API for views — zero view churn for selection. When a wave is upserted, the selected wave automatically reflects the new data because it's derived from the store. No more `if selectedWave?.id == wave.id { selectedWave = wave }` patching.

### RepoState forwarding properties

To minimize view churn, add computed properties on RepoState:

```swift
var waves: [WaveViewModel] { waveStore.ordered }
var waveGroups: WaveGroups { waveStore.groups }
```

Views that read `repoState.waves` and `repoState.waveGroups` continue to work unchanged. The `didSet` on the old `waves` array disappears — `@Observable` tracks access to `waveStore.ordered` and `waveStore.groups` directly.

### Migration of RepoState methods

| Method | Before | After |
|--------|--------|-------|
| `upsertWave()` | Private, linear scan | `waveStore.set()` — O(1) |
| `refreshWaves()` | Builds array, patches selected | `waveStore.setAll()`, selection auto-derives |
| `handleWaveEvent(.deleted)` | `waves.removeAll { }` + patch selected | `waveStore.remove()` + clear selectedWaveId |
| `handleWaveEvent(.updated)` | `upsertWave()` | `waveStore.set()` |
| `createWave()` pending insert | `waves.insert(pending, at: 0)` | `waveStore.set(pending)` |
| `createWave()` replace pending | `waves[index] = viewModel` | `waveStore.remove(pendingId); waveStore.set(viewModel)` |
| `configureMockWaves()` | Direct array assignment | `waveStore.setAll(mockWaves)` |
| `configureForUITest()` | `waves = []` | `waveStore.removeAll()` |

### Ordering in recompute()

The existing `buildWaveGroups` doesn't sort the input — it filters into buckets. Only `recentActivity` sorts by `lastActivityAt`. With dictionary storage, `waves.values` has no guaranteed order. The `ordered` property sorts all waves for consistent iteration in views like keyboard navigation (`waveGroups.allInOrder`).

Sort: blocked + pr + recentActivity (by lastActivityAt desc) + active + idle. Same effective order as today's `allInOrder`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep array, add index dictionary | Less refactoring — just add `waveIndex: [String: Int]` alongside the array | Two data structures to keep in sync. The index invalidates on every insert/remove. Doesn't simplify selection or prepare for optimistic patterns. |
| Make WaveStore a protocol | Enables mock store for testing | YAGNI. WaveStore is `@Observable` and in-memory — test it directly. Protocol adds indirection for no gain. |
| Move WaveStore into LoopflowCore | Lets Swift package tests cover it | WaveStore depends on `WaveGroups` grouping logic which uses `pendingPR` (a view concern). Keep it in Concerto. If we later need it in the package, promote the grouping logic then. |
| Full CQRS — separate read/write models | Clean separation, immutable views | Massive over-engineering for a single-user macOS app with ~50 waves max. Dictionary + recompute is the right complexity. |

## Key decisions

1. **Dictionary, not array.** O(1) lookup for upsert, remove, and selection. Items 02-05 all need `wave(for: id)` to apply optimistic mutations. Array scans don't scale to the access patterns we're building toward. Wave principle: "Single source of truth. WaveStore is canonical."

2. **Computed `selectedWave` on RepoState, not on WaveStore.** Selection is UI state — it belongs on the object views already observe (RepoState). WaveStore owns data; RepoState owns coordination. This follows the wave principle: "WaveStore is canonical. Views read from it."

3. **Forwarding properties on RepoState.** `var waves` and `var waveGroups` as computed properties pointing to the store. Zero view churn. We can remove these later if we want views to reference `waveStore` directly — but that's optional, not required.

4. **Closure-based status notifications.** WaveStore detects status changes; RepoState decides what to do about them (send notifications). This keeps WaveStore free of `NotificationService` dependency while still owning status tracking.

5. **`WaveGroups.empty` static property.** Small but useful — avoids the verbose initializer for the default value and prepares for the roadmap spec.

## Scope

- In scope: WaveStore class, dictionary storage, group computation, status tracking, selection derivation, forwarding properties, all call-site migration in RepoState + views
- Out of scope: optimistic mutations (item 02), event enrichment (item 03), RunStore (item 06), any new UI behavior

## Done when

1. `WaveStore.swift` exists at `Concerto/State/WaveStore.swift` with dictionary storage + group computation + status tracking
2. `RepoState` owns `let waveStore = WaveStore()` and delegates all wave state through it
3. `selectedWave` is computed from `selectedWaveId` via `waveStore.wave(for:)`
4. All views compile and behave identically (forwarding properties, no visual change)
5. `swift test --package-path swift` passes
6. `xcodebuild test -scheme Concerto -destination 'platform=macOS'` passes
