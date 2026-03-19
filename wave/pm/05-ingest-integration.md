---
asana_id: '1213718325451034'
linear_id: 53651936-b71f-45fa-a28c-c21c942bca78
---
# 05: Ingest auto-refresh

**Finish line:** `ingest` refreshes from the PM tracker before picking the next item when the wave has a `pm` block.

`lf ops pm pull` now exists as the deterministic remote-wins refresh (`ops/pm.rs::pm_pull`). The executor imports from the read/write provider at PR-oriented run start. But `lf ops ingest` still works off whatever currently happens to be in `wave/`. Manual roadmap pickup should see the same state the executor would see.

## What to build

1. Resolve the wave name and PM config as `ingest` already does for the filesystem lookup (`ops/ingest.rs::ingest`).
2. If a read/write PM provider is configured in the wave's `pm` block, call `pm_pull` to refresh local items from remote priority order.
3. After the refresh, run the existing `list_numbered_items` / move-to-`scratch/` logic against the updated wave directory.
4. Warn and continue on PM pull failure — the local roadmap is still better than blocking work.

This keeps PM planning authoritative without inventing a second sync path. Reprioritize items, update descriptions, add new items, delete stale ones — `ingest` should pick up the latest state before it decides what to move into `scratch/`.

## Constraints

- Reuse `pm_pull` from `ops/pm.rs` — no duplicated sync logic.
- If no `pm` block, ingest behaves exactly as before.
- Pull errors should warn, not block ingest — the local roadmap is still usable.
- Preserve existing ingest semantics once the wave directory has been refreshed.
- Stable run/item identity belongs to item 06; do not bolt on ad hoc lifecycle tracking here.

## Done when

- `lf ingest` on a wave with `pm` block refreshes items from the tracker
- `lf ingest` on a wave without `pm` block works unchanged
- A new item added in Asana/Linear appears as the next pick after `ingest`
- A reprioritized item in Asana/Linear changes the pick order
- A deleted remote item is no longer eligible to be ingested after the refresh
