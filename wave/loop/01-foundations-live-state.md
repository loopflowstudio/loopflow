---
status: proposed
seq: 1
---

# 01: Foundations + Live State

Establish explicit stack structure and remove stale PR-state assumptions.

## Estimated implementation size

~350-700 LOC across Rust storage/model/API paths plus sync integration.

## What exists after this step

- Every iteration has explicit ancestry metadata.
- Stack order is queryable without branch-name parsing.
- PR current state comes from live GitHub sync, not frozen snapshots.
- API DTOs can project accurate open/draft/merged state even after manual GitHub actions.

## Why this step is first

Queue lifecycle logic depends on two primitives:

1. A stable graph of iteration ancestry.
2. Reliable current PR state.

Without both, promotion, merge detection, and queue advancement become guesswork.

## Data model additions

### Run stack metadata

Store explicit lineage on each run (or a dedicated stack table keyed by run id):

- `parent_run_id: Option<RunId>`
- `parent_pr_number: Option<u64>`
- `stack_position: u32` (oldest = 0)
- `stack_group_id: String` (wave-scoped chain id)
- `stack_status: enum` (`active`, `superseded`, `merged`)

### Live PR state table

Store GitHub truth separately from run snapshot:

- `repo_id`
- `pr_number`
- `state` (`open`, `closed`, `merged`)
- `is_draft: bool`
- `head_ref`
- `head_sha`
- `base_ref`
- `updated_at`
- `merged_at: Option<DateTime>`
- `synced_at`

### Snapshot policy

Keep `run.snapshot.pr` unchanged as historical capture from run completion time.

## Migration and backfill

### New runs

- Populate stack metadata at run creation.
- `parent_run_id` points to most recent run in same wave.
- `stack_position` increments from previous max.

### Existing runs

Backfill in best-effort mode:

1. Group runs by wave.
2. Order by iteration index / created time.
3. Infer parent from previous iteration.
4. Mark inferred rows with `lineage_inferred = true` if confidence is partial.

No destructive rewrite of historical snapshots.

## Sync architecture

### Sync sources

- Primary: GitHub API / `gh pr list` + `gh pr view`.
- Trigger modes:
  - periodic polling
  - wave-scoped on-demand refresh
  - merge-triggered immediate refresh (used by step 02)

### Sync behavior

- Upsert by `(repo_id, pr_number)`.
- Preserve last successful sync timestamp.
- On partial failure, return stale marker instead of fabricating state.

### GitHub unavailable behavior

- Keep last synced values.
- Emit `is_stale: true` for affected projections.
- Avoid defaulting unknown to open.

## API changes

### Wave DTO

Expose stack + sync health:

- `open_pr_count` from live state
- `stack_count`
- `has_stale_pr_state`

### Run DTO additions

- `stack_position`
- `parent_run_id`
- `parent_pr_number`
- `live_pr_state`
- `live_pr_is_draft`
- `pr_state_stale`

### Ordering endpoint behavior

Add query support for chronological stack order so UI can render queue-first.

## Query helpers

Implement helpers in store/service layer:

- `list_stack_runs(wave_id) -> Vec<Run>` ordered oldest-first
- `find_next_unmerged_run(wave_id) -> Option<Run>`
- `find_descendants(run_id) -> Vec<Run>`
- `get_live_pr_state(repo_id, pr_number) -> Option<LivePrState>`

These become foundational for queue progression in step 02.

## Acceptance tests

### Storage tests

- Creating iteration N records parent linkage to N-1.
- Backfill does not overwrite existing explicit metadata.

### Sync tests

- Merged PR in GitHub transitions from open to merged in local state.
- Closed PR remains excluded from open counts.
- Unknown state is surfaced as unknown, not auto-open.

### API tests

- `open_pr_count` reflects live state, not snapshot-only state.
- Runs can be returned oldest-first with stable parent links.

## Rollout notes

- Ship model + sync first behind read-path fallback.
- Switch API projections to live state once sync reliability is verified.
- Keep temporary observability counters:
  - sync latency
  - stale projection rate
  - live-vs-snapshot mismatch count

## Non-goals in this step

- Draft/Ready promotion logic.
- Merge-triggered rebase logic.
- Combine reconciliation.
- Concerto queue UX.

## Done when

- Stack lineage is explicit and queryable.
- Current PR status in API reflects GitHub reality.
- Existing stale-count bug class is eliminated at source.
