# 06: Ingest auto-import

**Finish line:** `ingest` refreshes from the PM tracker before picking the next item when the wave has a `pm` block.

## What to build

Modify the `ingest` step to check for `pm` configuration:

1. Read wave YAML
2. If `pm` block present: run `lf ops pm import` (same as `import-pm` step)
3. Proceed with existing ingest logic — pick next unshipped item by priority rank

This means planning can happen entirely in the PM tool. Reprioritize items, update descriptions, add new items — `ingest` picks up the latest state without manual `import-pm` runs.

### Ship-roadmap flow update

`ship-roadmap` currently: ingest → kickoff → review-design → build → review → land

With PM integration, it naturally gains export at the end. The `ingest` step handles import. Consider adding `export-pm` as a post-land hook or appending to the flow when `pm` is configured.

## Constraints

- The import in ingest should be the same code path as `lf ops pm import` — no duplication
- If no `pm` block, ingest behaves exactly as before
- Import errors should warn, not block ingest — the local items are still valid

## Done when

- `lf ingest` on a wave with `pm` block refreshes items from the tracker
- `lf ingest` on a wave without `pm` block works unchanged
- A new item added in Asana/Linear appears as the next pick after `ingest`
- A reprioritized item in Asana/Linear changes the pick order
