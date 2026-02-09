---
status: done
seq: 3
---

# Event-driven sync

WebSocket events merge directly into WaveStore. Eliminates per-event GET requests and most `refreshWaves()` calls.

## What changed

### Rust (ws.rs)

`enrich_event()` intercepts wave lifecycle events before WebSocket serialization, fetches the current `WaveDto` via `build_wave_dto`, and injects it as a `"wave"` field alongside the original event fields. Non-wave events pass through unchanged.

### Swift (LocalEventService)

Six repetitive wave event parsing branches collapsed into one. All wave events now parse an optional `wave` JSON object via `LocalWaveService.parseWaveFromJSON`.

### Swift (WaveStore)

- `set()` checks `pendingMutations` and skips waves with in-flight mutations.
- `_set()` (private) bypasses the guard, used by `applyOptimistic` and `rollback`.
- `setAll()` preserves optimistic state for pending waves during full refreshes.
- `commitMutation()` clears the pending guard after API success.

### Swift (RepoState)

- `handleWaveEvent` prefers `event.wave` payload, falls back to `getWave()` GET.
- `refreshWaves()` removed from `runWave`, `stopWave`, `landWave`, `nextWave`.
- `commitMutation` calls added to `renameWave` and `updateWave` after API success.

### Swift (WaveDetailPanel)

`refreshRunsAndWaves()` eliminated. `collapsePRs` and `absorbIntoPR` call `fetchRuns()` directly.

## How it fits together

```
User action → RepoState.applyOptimistic → _set (bypasses guard) → UI updates instantly
                   ↓
             API call → commitMutation (clears guard)
                   ↓
         Daemon broadcasts Event → ws.rs enrich_event → WaveDto in JSON
                   ↓
         LocalEventService parses wave field → handleWaveEvent → set (guard allows)
```

## Key decisions

| Decision | Why |
|----------|-----|
| Enrich at WebSocket edge, not in Event enum | Keeps broadcast channel lightweight. Only WS clients pay the cost. |
| Fallback GET when `event.wave` is nil | Forward compatibility with older daemons; handles edge cases where enrichment fails. |
| `pendingMutations` as `Set<String>` | Simple whole-wave guard. Single user, local daemon. Per-field tracking is overkill. |
| `commitMutation` on API success, not event receipt | API confirmation is the definitive signal. Matching event data against optimistic data would be fragile. |
| Keep `refreshWaves()` for command palette + reconnection | Manual recovery mechanism. Costs nothing to maintain. |

## Known risks

- **`build_wave_dto` in the enrichment path** calls `spawn_blocking` for git state (~5ms per wave). Acceptable at current event rates, would need attention if volume increased.
- **`type.dropFirst(5)` in Swift parsing** assumes wave event types are prefixed with `"wave_"`. The `WaveEventType` enum guards against invalid values.
- **No timeout on pending mutations.** If an API call hangs, the pending guard blocks events indefinitely. Item 05 (responsive actions) could address this.
