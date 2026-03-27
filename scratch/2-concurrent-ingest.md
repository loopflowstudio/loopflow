# Concurrent Ingest

## Problem

With `workers: N`, multiple workers call `ingest` simultaneously. Today, `ingest` pulls from PM, picks the first local file by priority, and moves it to `scratch/`. Two workers hitting this at the same time will pick the same item — there's no coordination.

The PM provider is already the source of truth for assignment. `pm_try_claim` exists and implements claim-then-return. The missing piece is wiring: `ingest` doesn't call `pm_try_claim`, and the race between workers isn't handled for non-PM waves.

## Approach

Two layers of coordination, depending on whether PM is enabled:

### PM-backed waves: claim via provider

`ingest` calls `pm_try_claim` instead of local file picking when PM is enabled. `pm_try_claim` already:
1. Lists unassigned items from the provider
2. Walks them in priority order
3. Calls `claim_item` (assign to API token owner)
4. Returns the local filename on success, or tries the next item on failure

The race window is small: read-then-assign with retry on next item. No provider supports atomic conditional assignment (see de-risking below), but the failure mode is graceful — worst case, two workers claim the same item, one proceeds, the other's PR creation fails or produces a duplicate that's caught in review.

### Non-PM waves: filesystem advisory lock

For waves without PM, coordination happens via the local filesystem. Before moving a file from `wave/` to `scratch/`, `ingest` takes an advisory lock (`flock`) on a lockfile in `wave/<wave>/.ingest.lock`. The lock is held only during the pick-and-move operation (~milliseconds).

This works because all workers share the same main repo filesystem — worktrees are siblings but `wave/` lives in the main repo.

### Integration

```
ingest(repo, options, progress)
├── resolve wave
├── if PM enabled:
│   ├── pm_try_claim(repo, wave, progress) → Option<filename>
│   ├── if Some(filename): use it as the selected item
│   ├── if None: fall through to local pick (PM may be stale)
│   └── pm_pull to refresh local mirror
├── acquire flock on wave/<wave>/.ingest.lock
├── list_wave_items → pick highest priority
├── copy to scratch/, remove from wave/
├── release flock
└── if PM enabled: write claim status to frontmatter
```

The flock protects the local pick-and-move regardless of PM. PM claim is an additional coordination layer that prevents remote double-picks.

### Frontmatter status cache

After claiming, write to the item's frontmatter in `scratch/`:

```yaml
---
status: in-progress
claimed_by: <run-id>
claimed_at: 2026-03-27T16:00:00Z
---
```

This is local state only — a cache for `garden/scan` to read without hitting PM. The PM provider remains the source of truth.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Linear: conditional assignment? | No. `issueUpdate` with `assigneeId` is last-write-wins, no optimistic locking. `updatedAt` exists but isn't a precondition. | Read-then-assign with retry on next item. Two workers could claim the same issue; last write wins silently. Acceptable — the duplicate surfaces at PR creation. |
| Asana: conditional assignment? | No. `PUT /tasks/{id}` is last-write-wins. No ETag, no version field. Asana docs explicitly warn about concurrent overwrites. | Same as Linear. Read-then-assign with retry. |
| Notion: conditional assignment? | No conditional update, but Notion returns 409 Conflict on concurrent writes. `last_edited_time` exists but isn't a precondition. | Better than Linear/Asana — the 409 gives us a signal. Treat 409 as "someone else got it" and move to next item. |
| Workers share filesystem? | Yes. Worktrees are siblings but `wave/` is in the main repo. All workers see the same `wave/<wave>/` directory. | `flock` works for local coordination. |
| `pm_try_claim` already exists? | Yes, in `rust/loopflow/src/ops/pm.rs:912`. Lists unassigned, walks in order, calls `claim_item`, retries on failure. | Mostly wiring work — call it from `ingest`. |
| What about `workers > 1` on non-PM waves? | Filesystem lock is the only coordination. If two workers race past the lock (shouldn't happen with flock), one gets a "file not found" when removing — harmless. | flock is sufficient. Add error handling for "source file already moved." |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| lfd-side coordination (queue/lock in store) | Clean atomic claims, works without PM | Adds a new coordination API surface to lfd. Over-engineering for the race window size. PM providers already arbitrate; non-PM waves have the filesystem. |
| Optimistic concurrency via `updatedAt` check | Read item, check `updatedAt` matches before assign | No provider supports this as a precondition. We'd have to read-assign-read-verify — more calls, same race window, more complex. |
| Single ingest coordinator process | One process picks items, hands to workers | Architectural complexity for a narrow problem. Violates the design that workers are independent. |
| Do nothing (accept occasional duplicates) | Simplest | Acceptable for PM-backed waves (PR creation catches dupes), but non-PM waves would silently double-pick with no safety net. flock is trivial. |

## Key decisions

**PM claim is best-effort, not a hard gate.** If `pm_try_claim` fails entirely (network error, auth issue), ingest falls through to local file picking. This preserves the existing behavior — PM is an enhancement, not a requirement.

**flock over database locks.** All workers share the main repo filesystem. An advisory lock is simpler than threading lfd store coordination through the ingest path. The lock is held for ~1ms during pick-and-move.

**Notion 409 as a feature.** Unlike Linear and Asana (silent overwrite), Notion's 409 Conflict response is a genuine concurrency signal. The Notion `claim_item` implementation should treat 409 as "claimed by someone else" and return an error that triggers retry on the next item.

**Frontmatter status is write-only from ingest's perspective.** `ingest` writes status; `garden/scan` reads it. `ingest` never reads status to make decisions — PM assignment is the authority.

## Scope

- In scope:
  - Wire `pm_try_claim` into `ingest` for PM-backed waves
  - Add `flock` coordination for non-PM concurrent picks
  - Write `status`/`claimed_by`/`claimed_at` to frontmatter after claim
  - Handle Notion 409 as "already claimed" in `claim_item`
  - Handle "source file already moved" gracefully in non-PM path
  - Tests for concurrent ingest (two threads, same wave, no double-pick)

- Out of scope:
  - lfd-side coordination API
  - New PM provider trait methods
  - Changes to `garden/scan` (reads frontmatter — already works)
  - `workers > 1` on crons (separate from `3-wave-scheduling`)

## Done when

- `ingest` calls `pm_try_claim` when PM is enabled, before local file picking
- `flock` prevents two workers from picking the same local file
- Notion `claim_item` treats HTTP 409 as "already claimed"
- Frontmatter in `scratch/` includes `status: in-progress`, `claimed_by`, `claimed_at`
- `cargo test` passes with a test showing two concurrent ingest calls on the same wave pick different items
- Works with Linear, Asana, and Notion (all three `claim_item` implementations handle the race)
