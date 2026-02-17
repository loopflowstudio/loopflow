# wave/ux wrap-up and clear plan

## Goal

Close `wave/ux` cleanly by separating shipped work from stale/superseded proposals, leaving only real remaining work in active waves.

## Audit: roadmap vs shipped

### Snappy roadmap (`wave/ux/README.md`, items 01–06)

All six items are shipped in code:

1. **WaveStore** — `swift/Concerto/State/WaveStore.swift` is the canonical in-memory source for waves.
2. **Optimistic data mutations** — `RepoState.renameWave` and `RepoState.updateWave` use optimistic helpers + rollback.
3. **Event-driven sync** — event subscription updates `WaveStore` directly (`startEventSubscription`, `handleWaveEvent`) and no longer does per-event full list refresh.
4. **Optimistic create/delete** — pending IDs + insert/replace/delete rollback patterns are implemented in `RepoState` + `WaveStore`.
5. **Responsive actions** — run/stop/next/restart use `optimisticAction` transitional updates.
6. **RunStore** — `swift/Concerto/State/RunStore.swift` exists and is used by detail views; tests cover store behavior.

### Wave specs launcher proposal (`wave/ux/wave-specs-launcher.md`)

The original proposal is mostly superseded by shipped **wave schema** work:

- Shipped: schema discovery + resolution (`/v0/wave/schemas`), provenance in storage, schema-based wave creation, Concerto schema instantiation UX.
- Not implemented from original doc (and now out-of-date): markdown frontmatter specs on `wave/*.md`, `.lf/sprint.yaml` subsets, `/waves/launchable` endpoint, dedicated checkbox batch-launch view for wave-item files.

## What is actually left

No remaining actionable work in `wave/ux` for the shipped Snappy sequence.

Only cleanup remains:

1. Mark `wave/ux` as closed/shipped at the README level.
2. Remove stale `wave-specs-launcher` proposal from active wave backlog (it no longer matches implemented architecture).
3. Keep any future launcher planning in a fresh wave doc when/if product direction reopens that scope.

## Clear plan

1. Update `wave/ux/README.md` to be a closure doc:
   - explicit shipped status for items 01–06
   - note that this wave is complete/cleared
2. Delete `wave/ux/wave-specs-launcher.md` from active wave planning.

## Done when

- `wave/ux` contains no open/proposed implementation items.
- README states closure clearly.
- Remaining UX work (if any) must be reintroduced as new scoped wave items, not carried as stale proposals.
