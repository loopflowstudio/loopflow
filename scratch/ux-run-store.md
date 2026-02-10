---
status: in-progress
seq: 6
---

# RunStore: cached wave runs

## Problem

Every time a user switches to the Runs tab (or switches waves, or a wave's status changes), `WaveDetailPanel` fires `fetchRuns()` — a full network round-trip to `GET /api/wave_runs`. The tab shows a loading spinner while waiting. For a local daemon the latency is ~50ms, but it's visible and it compounds: switch tabs, see spinner, wait, switch back, spinner again.

Meanwhile, WebSocket events already tell us when runs start (`wave.started`) and stop (`wave.stopped`/`wave.updated`), but we ignore them for run data. The Runs tab is blind to live updates.

## Approach

**New file: `Concerto/State/RunStore.swift`** — a flat dictionary store keyed by wave ID, following WaveStore's `@Observable` pattern.

**Storage:** `[String: [WaveRun]]` keyed by wave ID. Runs for a given wave are always sorted newest-first. No secondary index by run ID — runs are only queried by wave.

```swift
@MainActor
@Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]] = [:]

    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func upsertRun(_ run: WaveRun)
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

**Stale-while-revalidate:** When `WaveDetailPanel` appears or the wave changes, it calls `repoState.loadRuns(for: waveId)`. This method:
1. Returns immediately — cached runs (or empty) are already in RunStore, UI reads them
2. Fires background fetch via `waveService.listWaveRuns(waveId:)`
3. Replaces cache with response via `runStore.setRuns(for:_:)`
4. UI auto-updates via `@Observable`

First load still shows the spinner (empty cache). Every subsequent load shows stale data instantly, then silently refreshes.

**Event-driven updates:** `handleWaveEvent` already handles `.started`/`.stopped`/`.updated` by refreshing the wave. Extend it: on `.started`, fetch the new run and upsert it into RunStore. On `.stopped`/`.updated`, re-fetch runs for that wave (the run's status/endedAt changed). This is cheap — one GET per event, and events are infrequent.

Why fetch instead of constructing from the event? The `WaveEvent` has `waveRunId` but not the full run payload (flow, branch, worktree, PR, etc). The API returns everything. One extra GET is simpler than enriching events server-side.

**OutputBuffer cleanup on wave delete:** When `deleteWave` succeeds (after `commitMutation`), clear OutputBuffer for that wave. This is a one-liner — `outputBuffer.clearOutput(for: wave.id)` — but it requires OutputBuffer to be accessible from RepoState. Pass it to `connectLfd` already; store a weak reference.

**Integration with WaveDetailPanel:** Remove `@State private var waveRuns` and `isLoadingRuns`. Read runs from `repoState.runStore.runs(for: wave.id)` directly. The `runsRefreshKey` change-detection hack goes away — RunStore handles freshness. Collapse/absorb operations call `repoState.loadRuns(for:)` after completion to pick up PR changes.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Enrich run events server-side (like wave events) | Avoids extra GET on event | Adds complexity to ws.rs for marginal gain. Run events are rare (~1/min during active waves). One GET is fine. |
| Per-run dictionary (`[String: WaveRun]` by run ID) + wave index | O(1) upsert by run ID | Over-engineered. Runs are only queried by wave. A flat array per wave is simpler and fast enough (50 items max). |
| No event integration — just cache + fetch on tab switch | Simpler, no event handling | Misses the key UX win: seeing a new run appear live while watching the Runs tab. |
| Full optimistic pattern (pendingMutations etc) | Consistency with WaveStore | Runs are never mutated by the user. No optimistic writes needed. Just cache and replace. |

## Key decisions

1. **Array per wave, not dictionary per run.** RunStore is read-heavy and append-mostly. Runs are only queried by wave ID. A sorted array per wave is the simplest structure that works. Cap at 50 (existing API limit).

2. **Fetch on event, don't construct.** When `wave.started` fires, fetch the run from the API rather than trying to build a `WaveRun` from event fields. The event doesn't carry the full payload. One GET per run start is invisible to the user.

3. **No optimistic mutations.** Unlike WaveStore, RunStore has no user-initiated writes. All updates come from the server (API fetch or event-triggered fetch). No pendingMutations, no rollback.

4. **Weak OutputBuffer reference in RepoState.** RepoState already receives OutputBuffer in `connectLfd`. Store `weak var outputBuffer: OutputBuffer?` so `deleteWave` can clear output. Follows the wave's "In-memory only" principle — deleting a wave clears all traces.

5. **`upsertRun` for event-driven single-run updates.** When a run event arrives and we fetch the run, upsert it into the cached array (replace if same ID exists, append if new). This avoids re-fetching the full list for every event.

## Scope

In scope:
- RunStore with `setRuns`, `upsertRun`, `runs(for:)`, `clear`
- RepoState owns RunStore, exposes `loadRuns(for:)`
- Event handler updates RunStore on wave.started/stopped/updated
- WaveDetailPanel reads from RunStore instead of local state
- OutputBuffer cleared on wave delete
- Tests for RunStore

Out of scope:
- Server-side run event enrichment (not needed for 50ms local daemon)
- Run-level optimistic mutations (no user writes to runs)
- OutputBuffer restructuring (it works fine as-is, just needs cleanup on delete)
- Persisting runs across app restarts (in-memory only per wave principles)

## Done when

1. Switching to the Runs tab shows cached data instantly (no spinner after first load)
2. Starting a wave makes the new run appear in the Runs tab without manual refresh
3. A run completing updates its status/duration in the Runs tab live
4. Deleting a wave clears its runs from RunStore and its output from OutputBuffer
5. `swift test --package-path swift` passes
6. `xcodebuild test` passes
