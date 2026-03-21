# Concurrent Ingest

**Finish line:** Multiple workers in a pool call `ingest` and the PM provider (Linear/Asana) arbitrates who gets which item. No double-picks.

## Context

With `workers: N` (item 02a), multiple workers call `ingest` simultaneously. `ingest` already talks to PM providers to pick items. The PM provider is the natural arbiter — it's already the source of truth for "who's working on what."

Depends on:
- 02a (worker pools exist)

## The approach

Leverage PM provider assignment as the coordination mechanism. When a worker calls `ingest`:

1. Query PM provider for the next unassigned item in priority order
2. Atomically assign it to this worker's run ID
3. If assignment fails (someone else claimed it), move to the next item
4. Once claimed, write local frontmatter status as a cache

The PM provider handles the race. Loopflow doesn't need its own locking or coordination API.

### What needs validation

- **Linear**: Does the API support conditional assignment (assign only if unassigned)? Or do we need to read-then-assign and handle conflicts?
- **Asana**: Same question. If neither supports true atomic claims, a read-then-assign with conflict detection on the second read is likely good enough — the race window is small and the failure mode is graceful (two workers start the same item, one fails at PR creation).

### Frontmatter as local cache

Items get `status: available | in-progress | done` in frontmatter with the claiming run's ID. This is local state, not the source of truth — it survives lfd restarts and gives `garden/scan` something to read without hitting the PM API.

## Done when

- `ingest` claims items through the PM provider before starting work
- Multiple concurrent workers don't grab the same item
- Frontmatter reflects claim status as local cache
- Works with at least one PM provider (Linear or Asana)
