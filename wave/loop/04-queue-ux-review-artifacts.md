---
status: proposed
seq: 4
---

# 04: Queue UX + Review Artifacts

Make landing intent obvious in Concerto and remove merge dependence on tracked scratch docs.

## Estimated implementation size

~250-650 LOC across Swift queue views and PR artifact publishing paths.

## What changed after 01 + 02 shipped

- Backend already supports queue-first ordering (`order=stack`) and queue role projection.
- Run DTOs already expose `queue_role`, `queue_block_reason`, `queue_blocked_at`, and `next_action`.
- Scratch-clean promotion gating already exists (`scratch_dirty` -> blocked with `next_action=resolve_conflict`).
- Wave and run projections now expose stale PR-state metadata that UI must handle explicitly.
- `QueueRole` has five values (Ready, Draft, Blocked, Merged, Superseded) — badge mapping is 1:1, no client-side inference needed.
- `QueueNextAction` has four values (OpenPr, ResolveConflict, CombinePrs, AwaitMerge) — maps directly to primary actions in the UI.
- `LivePrSnapshot` caching means API responses include fresh-enough PR state without requiring the UI to poll GitHub separately.
- Phase 04 is almost entirely a *consumer* of shipped backend semantics. No new queue logic needed on the backend — only review artifact publishing and the Swift UI layer.

## What exists after this step

- Runs tab defaults to queue-first workflow (oldest-first landing path).
- Users can see `ready`, `draft`, `blocked`, `merged`, `superseded` at a glance.
- Review summary is published on PR surfaces (managed comment/body block).
- Ready promotion enforces scratch-clean state without post-merge cleanup commits.

## Queue-first Concerto behavior

### Primary question

UI should answer: **"What should I land next?"**

### Default ordering

- Oldest-first queue at top.
- Reverse-chron timeline remains available as secondary history view.
- Use existing `order=stack` backend support as the default list mode.

### Role badges

- Ready to land
- Waiting in queue
- Blocked (rebase conflict)
- Merged
- Superseded (combined)

### Primary actions

- `Open PR`
- `Combine PRs`
- `Resolve Blocked`
- `Refresh PR State`

## UX constraints

- No stale badge rendering; consume live-state-backed DTO fields.
- Surface stale PR state as explicit degraded mode, not silent omission.
- Keep current run detail/log surfaces intact.
- Preserve accessibility conventions from `VISUAL_DESIGN.md`.

## Review artifact publishing model

### Principle

`scratch/` is ephemeral working memory. PR-visible review context must live in managed PR output.

### Canonical storage

Store review summary content in run metadata.

### Publishing targets

Preferred:

- managed bot comment (idempotent update)

Fallback:

- marker-delimited PR body block:
  - `<!-- lf:auto-review:start -->`
  - `<!-- lf:auto-review:end -->`

### Clobber prevention

- only replace managed block/comment
- never overwrite manual user text outside managed area
- preserve custom PR descriptions

## Scratch-clean gate in queue workflow

Backend already enforces this gate during queue reconciliation.

UI requirements:

- show blocked reason when `queue_block_reason=scratch_dirty`
- surface `next_action=resolve_conflict` as the primary remediation path
- avoid suggesting "land now" actions while the gate is active

## GitHub-first merge compatibility

Landing may occur from GitHub UI directly.

Expected behavior:

1. Merge detector advances queue.
2. Concerto updates queue roles.
3. Review artifact links remain valid.
4. No manual `lf ops` requirement.

## API contract needed by UI

### Already shipped

- `queue_role` (Ready, Draft, Blocked, Merged, Superseded)
- `queue_block_reason` (MissingPr, WaveRunning, ScratchDirty, RebaseConflict, PromotionFailed)
- `queue_blocked_at`
- `next_action` (OpenPr, ResolveConflict, CombinePrs, AwaitMerge)
- wave-level `has_stale_pr_state`
- `open_pr_count`, `stack_count`

### Still needed for full Phase 04 + 03 UX

- `superseded_by_pr` (from Phase 03 combine events)
- `combined_pr` (from Phase 03 combine events)

Review artifact fields:

- `review_artifact_status` (`published`, `stale`, `retry_needed`)
- `review_artifact_url`
- `review_artifact_updated_at`

## Test plan

### UI logic tests

- queue ordering displays oldest-first by default
- role badges map correctly from DTO
- blocked state shows actionable path (`resolve_conflict`) for scratch/rebase failures
- stale-state indicator appears when live PR state is unavailable

### Publishing tests

- bot comment updates are idempotent
- marker block updates preserve non-managed body text
- publish failures mark retry-needed without corrupting content

### Promotion gate tests

- blocked scratch/rebase status is rendered from DTO without client-side re-implementation
- landing actions remain hidden/disabled for blocked queue-head runs

## Rollout strategy

1. Enable queue-first as default using existing API fields.
2. Add stale/degraded-state UX and blocked remediation surfaces.
3. Migrate review summary publish path from scratch doc dependence.
4. Remove legacy UI assumptions tied to reverse-chron-only flow.

## Open questions

- Should review summaries live in run metadata only, or also persist last-published artifact pointers for fast UI fetch?
- For publish failures, is inline retry in Concerto enough, or do we need daemon-side retry scheduling in this phase?

## Resolved questions (from 01+02)

- **Does the UI need to compute queue roles?** No — roles are fully projected by the backend. UI consumes directly.
- **How does the UI handle stale PR state?** `has_stale_pr_state` is already on the wave DTO. Show degraded-mode indicator, not an error.
- **What ordering does the queue view use?** `order=stack` is already implemented and returns oldest-first.

## Try it

- On a wave with 3 stacked runs, confirm queue-first view answers "what lands next" without opening run details.
- Trigger a scratch-dirty block and verify the UI shows `Resolve Blocked` with no false-ready affordance.
- Merge queue-head from GitHub UI and confirm queue badges update within one reconciliation cycle.

## Non-goals

- Full redesign of all Concerto wave surfaces.
- Rich templating framework for review summary markdown.
- Multi-PR parallel reviewer workflow optimization.

## Done when

- Concerto makes queue progression clear in one glance.
- Review context survives rebases/merges without tracked scratch files.
- GitHub Land button remains first-class with no hidden workflow penalties.
