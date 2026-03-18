---
pm_id: '1213718325451034'
---
# 06: Ingest auto-import

**Finish line:** `ingest` refreshes from PM before picking the next item when the wave has a `pm` block.

Today `rust/loopflow/src/ops/ingest.rs` is still a pure local fast-path: it lists numbered files, picks the lowest prefix, copies it to `scratch/`, and deletes the source file. That stays the fallback. Once item 05 lands, linked waves should run the exact PM import helper before selection so remote reprioritization or new items show up without a manual sync step.

## What to build

1. Read the wave config before local selection.
2. If a `pm` block is present, run the same import implementation as `lf ops pm import`.
3. Continue with the existing ingest selection logic on the refreshed local files.
4. If no `pm` block is present, behave exactly as today.

### Ship-roadmap flow update

`ship-roadmap` should not grow a second PM codepath. Either:
- let `ingest` own the import side and add PM export at the back of the flow, or
- route linked waves through the shared `pm-sync` building blocks

Keep the sync wiring in one place so non-PM waves stay untouched.

## Constraints

- Reuse the exact import helper from item 05; no duplicated sync logic.
- Import failures should warn and fall back to the local roadmap instead of blocking ingest.
- Preserve today's fast-path behavior for unlinked waves.

## Done when

- `lf ingest` on a linked wave refreshes local items before choosing one
- `lf ingest` on an unlinked wave behaves exactly as before
- A newly added Asana/Linear item can become the next pick after ingest
- Remote reprioritization changes which file ingest chooses
