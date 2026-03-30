# Concurrent Ingest

## Problem

With `workers: N`, multiple workers call `ingest` simultaneously. Today, `ingest` pulls from PM, picks the first local file by priority, and moves it to `scratch/`. Two workers hitting this at the same time will pick the same item — there's no coordination.

The PM provider is already the source of truth for assignment. `pm_try_claim` exists and implements claim-then-return. The missing piece is wiring: `ingest` doesn't call `pm_try_claim`, and the race between workers isn't handled for non-PM waves.

## Approach

Two layers of coordination, depending on whether PM is enabled:

### Selection order: not from numeric filename prefixes

The worker-striping idea assumes one canonical ordering for local roadmap items. That ordering should be explicit and stable, but it does not need to live in the filename.

Preferred direction: stop treating `1-foo.md` / `2-bar.md` style prefixes as the source of truth for priority. The number in the title is useful today for ordering, but it leaks implementation detail into the user-facing item name.

Canonical local ordering should be:

1. `priority`
2. `rank` within that priority bucket
3. alphabetical tiebreaker if ranks are equal or unknown

Known rank beats unknown rank within the same priority bucket.
`rank` is a float in the range `0..=1`, where lower values come first.

That keeps priority explicit without baking numbers into the filename. The likely shape is frontmatter such as:

```yaml
---
priority: high
rank: 0.2
---
```

The core concurrency design only needs a deterministic sorted list that every worker can compute the same way.

### Provider-native ordering, normalized into frontmatter

For PM-backed waves, ordering should live in the most natural representation each provider already offers, then be translated into local frontmatter.

- **Asana** — use a project custom field for `priority`; if we need within-bucket ordering, look for the most natural Asana field for rank and mirror it locally
- **Linear** — use Linear's native ordering signals (`prioritySortOrder`, `sortOrder`) and translate them into local `priority` + `rank`
- **Notion** — use database properties (`Priority`, plus a rank/order property if needed) and translate them into local `priority` + `rank`

The local markdown file becomes the normalized shape regardless of provider:

```yaml
---
priority: high
rank: 0.2
asana_id: ...
linear_id: ...
notion_id: ...
---
```

That keeps `ingest` simple: it reads one canonical local representation. Provider adapters handle the translation in and out.

### PM-backed waves: claim via provider

`ingest` calls `pm_try_claim` instead of local file picking when PM is enabled. `pm_try_claim` already:
1. Lists unassigned items from the provider
2. Walks them in priority order
3. Calls `claim_item` (assign to API token owner)
4. Returns the local filename on success, or tries the next item on failure

The race window is small: read-then-assign with retry on next item. No provider supports atomic conditional assignment (see de-risking below), but the failure mode is graceful — worst case, two workers claim the same item, one proceeds, the other's PR creation fails or produces a duplicate that's caught in review.

### Non-PM waves: worker-index striping

For waves without PM, coordination can happen by giving each concurrent worker a stable ordinal within the wave activation (`0..workers-1`). Each worker reads the same priority-sorted local item list and picks the item at its ordinal:

- worker 0 → highest priority
- worker 1 → second highest
- worker 2 → third highest

This removes the need for a lock in the common case. The design only works if loopflow exposes a real worker ordinal to `ingest`; a unique run ID is not enough on its own because the mapping needs to be deterministic across the concurrent workers for one activation burst.

### Integration

```
ingest(repo, options, progress)
├── resolve wave
├── if PM enabled:
│   ├── pm_try_claim(repo, wave, progress) → Option<filename>
│   ├── if Some(filename): use it as the selected item
│   ├── if None: fall through to local pick (PM may be stale)
│   └── pm_pull to refresh local mirror
├── for non-PM waves: list_wave_items → pick item at worker ordinal
├── copy to scratch/, remove from wave/
└── if PM enabled: write claim status to frontmatter
```

PM claim remains the coordination layer for PM-backed waves. Non-PM waves rely on deterministic striping across the concurrently spawned workers.

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
| Workers need a stable ordinal? | Yes. The striping approach only works if concurrent runs can see a deterministic worker index for the same activation burst. `LF_RUN_ID` already exists, but that's unique per run, not an ordinal. | Either add a `LFD_WORKER_INDEX`/similar env var, or fall back to lock-based coordination. |
| Where does item order live? | Current code derives order from filename prefixes (`1-...`, bucket prefixes). The new shape wants canonical ordering independent from the human-facing title. | Move ordering into metadata: `priority` first, then known `rank`, then alphabetical as the fallback/tiebreaker. |
| How should PM providers represent ordering? | Each provider has its own natural shape. Asana already uses a priority custom field; Linear exposes native sort fields; Notion uses database properties. | Use provider-native storage remotely, then normalize all of them into local frontmatter as `priority` + `rank`, where `rank` is a float from 0 to 1. |
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

**Worker-index striping over global locking, if the runtime can expose the index cleanly.** The selection rule is simple and deterministic, but it depends on a real worker ordinal from the scheduler. If the runtime cannot expose that cleanly, fall back to a short-lived filesystem lock.

**Ordering should move out of the filename prefix.** The ingest algorithm should sort by `priority`, then by known `rank` within priority, then alphabetically when ranks are equal or unknown — not by numbers embedded in the title. `rank` is a normalized float from 0 to 1, and lower wins.

**Remote representation is provider-native; local representation is normalized.** Asana/Linear/Notion can each store ordering in the way that fits their model best, but local roadmap files should all expose the same `priority` + `rank` frontmatter for ingest and fallback behavior.

**Notion 409 as a feature.** Unlike Linear and Asana (silent overwrite), Notion's 409 Conflict response is a genuine concurrency signal. The Notion `claim_item` implementation should treat 409 as "claimed by someone else" and return an error that triggers retry on the next item.

**Frontmatter status is write-only from ingest's perspective.** `ingest` writes status; `garden/scan` reads it. `ingest` never reads status to make decisions — PM assignment is the authority.

## Scope

- In scope:
  - Wire `pm_try_claim` into `ingest` for PM-backed waves
  - Add worker-index coordination for non-PM concurrent picks, or fall back to `flock` if no stable worker ordinal is available
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
  - Non-PM concurrent ingest avoids double-picks, either via worker-index striping or a lock fallback
- Notion `claim_item` treats HTTP 409 as "already claimed"
- Frontmatter in `scratch/` includes `status: in-progress`, `claimed_by`, `claimed_at`
- `cargo test` passes with a test showing two concurrent ingest calls on the same wave pick different items
- Works with Linear, Asana, and Notion (all three `claim_item` implementations handle the race)
