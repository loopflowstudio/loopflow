---
asana_id: '1213718325451034'
linear_id: eb086ebe-2987-4cf3-aba1-147facc51145
---
# 06: Ingest auto-import

**Finish line:** `ingest` refreshes from the PM tracker before picking the next item when the wave has a `pm` block.

## What to build

Modify the `ingest` step to check for `pm` configuration:

1. Read wave YAML
2. If `pm` block present: run the same import code path as `lf ops pm import`
3. Proceed with existing ingest logic — pick the next unshipped item by priority rank

This keeps PM planning authoritative without inventing a second sync path. Reprioritize items, update descriptions, add new items — `ingest` should pick up the latest state without a manual `import-pm` run.

### Integration with land

The pm-sync design establishes that `lf ops land` runs sync for each wave that has a `pm` block before merge. This ensures main is always consistent with the remote PM state. The three-way diff inputs are clean: main = base, branch = local, remote = current.

```
ingest → sync(wave) → agent works → PR → land → next cycle syncs again
```

If the remote changes again between sync and land, those changes get caught on the next sync. The invariant is eventual consistency, not perfect-at-every-moment.

### Ship-roadmap flow update

`ship-roadmap` currently: ingest → kickoff → review-design → build → review → land

With PM integration, ingest handles the pull and the flow needs an export at the back when `pm` is configured. Keep that conditional wiring in one place so `ship-roadmap` without PM stays unchanged.

## Constraints

- Reuse the exact import implementation from `lf ops pm import` — no duplicated sync logic.
- If no `pm` block, ingest behaves exactly as before.
- Import errors should warn, not block ingest — the local roadmap is still usable.

## Done when

- `lf ingest` on a wave with `pm` block refreshes items from the tracker
- `lf ingest` on a wave without `pm` block works unchanged
- A new item added in Asana/Linear appears as the next pick after `ingest`
- A reprioritized item in Asana/Linear changes the pick order
