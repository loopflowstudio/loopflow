# Concerto UX: Snappy

Make every interaction in Concerto feel instant. In-memory store with optimistic updates, event-driven sync.

## Sequence

Each item is a self-contained change (~200-400 LOC). Implement in order — each builds on the last.

| # | Item | What changes |
|---|------|-------------|
| 01 | WaveStore | Extract wave state into dedicated `@Observable` store |
| 02 | Optimistic data mutations | Rename + config updates apply locally first |
| 03 | Event-driven sync | WebSocket events merge into store; kill `refreshWaves()` |
| 04 | Optimistic create/delete | Generalize existing `createWave` pattern to delete + clone |
| 05 | Responsive actions | Run/stop/land/next with transitional states |
| 06 | RunStore | Same pattern for wave runs and output lifecycle |

## Architecture

```
UI ←→ RepoState ←→ WaveStore (source of truth) ←→ LocalWaveService (HTTP)
                        ↑
                   LocalEventService (WebSocket)
```

**Before:** Every mutation round-trips to the daemon, then `refreshWaves()` re-fetches the entire wave list.

**After:** Mutations apply to WaveStore immediately. Background sync via API + WebSocket events. UI never waits for the network.

## Principles

- **In-memory only.** No SQLite, no persistence. The daemon is local; repopulate on connect.
- **Optimistic for data, responsive for actions.** Rename applies instantly (the new value IS correct). Run shows "starting..." (the outcome isn't known yet).
- **Single source of truth.** WaveStore is canonical. Views read from it. Events merge into it. No parallel arrays.
- **Last-write-wins.** Single user, local daemon. No conflict resolution needed yet.

## Item format

```yaml
---
status: todo | in-progress | done
seq: 1-6
---
```

## Reference

Architecture: `reports/concerto/`
Visual design: `VISUAL_DESIGN.md`
Current state: `swift/Concerto/State/RepoState.swift`
