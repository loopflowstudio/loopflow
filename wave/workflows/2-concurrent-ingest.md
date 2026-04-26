---
asana_id: '1213869424664965'
---
# Concurrent ingest

**Needs:** workflows/3-wave-scheduling

**Finish line:** Multiple workers in a pool call `ingest` and the PM provider (Linear/Asana/Notion) arbitrates who gets which item. No double-picks when the provider can identify the claimant; graceful fallback when it cannot.

## Context

With `workers: N`, multiple workers call `ingest` simultaneously. `ingest` already talks to PM providers to pick items. The PM provider is the natural arbiter — it is already the source of truth for “who is working on what.”

## The approach

Leverage PM provider assignment as the coordination mechanism. When a worker calls `ingest`:

1. Query the PM provider for the next unassigned item in priority order
2. Claim it for this run
3. Verify the claim stuck when the provider supports that check
4. If the claim lost a race, move to the next item
5. Once claimed, write local frontmatter status as a cache

The provider handles the race. Loopflow does not need a second locking system.

## What needs validation

- **Linear** — optimistic claim plus response verification
- **Asana** — claim, then re-read to confirm the assignee is still us
- **Notion** — best-effort only; status can change, but the claimant is not explicit

## Frontmatter as local cache

Items get `status: available | in-progress | done` in frontmatter with the claiming run's ID when possible. This is local state, not the source of truth — it survives lfd restarts and gives `scan` something to read without hitting the PM API.

## Done when

- `ingest` claims items through the PM provider before starting work
- Multiple concurrent workers do not grab the same item when the provider supports claimant verification
- Frontmatter reflects claim status as local cache
- Best-effort providers fail gracefully instead of pretending they are atomic
