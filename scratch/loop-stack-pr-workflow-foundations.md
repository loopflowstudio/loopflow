# Loop Stack PR Workflow — Foundations Plan

Current implementation plan for backend foundations behind stacked wave PR lifecycle.

## Why this exists

Looping waves currently create stacked PRs but do not maintain stack state after creation.

Primary failures to fix:

- PR state in run snapshots drifts from live GitHub state.
- Queue ordering and promotion are implicit instead of explicit.
- Merging a queue-head PR does not deterministically advance the next run.
- Stack lineage is inferred from branch naming instead of persisted metadata.

## Decisions (v1)

1. **Keep stacking.** New iterations continue branching from previous iteration HEAD.
2. **Make lineage explicit.** Persist parent links and stack position metadata on runs.
3. **Use live PR state as truth.** Snapshot PR data remains historical only.
4. **Draft-first queue.** Exactly one oldest eligible run is Ready at a time.
5. **Lazy advancement on merge.** Rebase only next-in-line draft onto `origin/main`.
6. **Blocked is explicit.** Rebase conflicts persist queue block reason; no silent retries.

## Foundations scope

### In scope

- Run lineage storage and API exposure.
- Live PR state storage and sync.
- Queue projection fields and single-ready invariant.
- Merge detection pipeline (webhook + polling fallback) to shared advancement handler.
- Lazy rebase + promotion path for next run.

### Out of scope (later wave items)

- Full Combine reconciliation model.
- Queue-first Concerto UX polish.
- Review artifact publishing migration.

## Current baseline already landed

- `open_pr_count` now dedupes by PR number across runs.
- Unknown PR state (`None`) is treated as not-open in route-level open-state checks.
- CI hook target matching shares the same open-state predicate.

These are correctness guardrails, not the full live-state solution.

## Implementation phases

### Phase A — Storage + model

- Add run lineage fields (`parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`).
- Add live PR state table keyed by `(repo, pr_number)` with state/draft/head/base timestamps.
- Backfill lineage best-effort for existing runs.

### Phase B — Sync + projections

- Add periodic and on-demand GitHub PR sync.
- Compute `open_pr_count` from live PR state table (not snapshots).
- Add stale-sync marker (`has_stale_pr_state`) in wave DTO.
- Expose run-level lineage + queue projection fields.

### Phase C — Queue core

Queue rules:

1. Exclude merged/closed/superseded runs.
2. Oldest open non-blocked run becomes Ready.
3. Deeper open runs remain Draft.
4. If queue-head is blocked, do not promote deeper runs.

### Phase D — Merge advancement

- Detect merges via webhook (`pull_request.closed` + `merged=true`) and polling fallback.
- Shared idempotent handler:
  - refresh live state
  - mark merged run
  - find next draft
  - attempt lazy rebase + force-with-lease push
  - promote draft to Ready on success
  - persist Blocked role/reason on conflict
- Add wave-scoped lock + processed-event guard `(wave_id, pr_number, merged_at)`.

## Acceptance checklist

- [ ] New runs persist explicit lineage.
- [ ] Stack ordering no longer depends on parsing branch names.
- [ ] GitHub merge state sync updates backend projections.
- [ ] Exactly one open run is Ready during normal operation.
- [ ] Queue-head merge advances next run or marks queue blocked.
- [ ] Open PR counting does not treat unknown state as open.

## Test plan

### Rust unit/integration

- lineage write + backfill behavior
- live sync mapping
- queue role projection invariants
- merge handler idempotency
- blocked-state persistence on rebase conflict

### E2E smoke

1. create stacked wave iterations
2. verify draft/ready projection
3. simulate merge of queue-head PR
4. verify single-ready invariant after advancement

## Open policy question

- When `snapshot.pr.number` is `None` but state is `open`/`draft`, should wave-level open PR counts include it or ignore it to avoid overcount drift until live sync is authoritative?
