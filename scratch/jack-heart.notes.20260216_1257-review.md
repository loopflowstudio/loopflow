# Review: Live PR State + Stack Lineage Foundations

## What was implemented

Added explicit stack lineage tracking and live GitHub PR state to the wave run system. Previously, stack ordering was inferred from branch names and PR state came from frozen snapshots that drifted from reality.

Concrete changes:

- **Stack metadata on wave runs**: `parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`, `stack_status`, `lineage_inferred` — all persisted in SQLite/Postgres.
- **Live PR state table**: `live_pr_states` keyed by `(repo_id, pr_number)`, synced on-demand from GitHub REST API.
- **Migration 005**: Adds lineage columns to `wave_runs`, backfills parent links from existing data using window functions, creates `live_pr_states` table.
- **GitHub API client**: `fetch_pull_request()` for per-PR live state; `github_repo_from_local()` for resolving repo → full name.
- **Wave DTO projection**: `open_pr_count` now derived from live state (not snapshots), plus `stack_count`, `has_stale_pr_state`.
- **Run DTO additions**: `stack_position`, `parent_run_id`, `parent_pr_number`, `live_pr_state`, `live_pr_is_draft`, `pr_state_stale`.
- **Query helpers**: `list_stack_runs`, `find_next_unmerged_run`, `find_descendants`, `get_live_pr_state`, `upsert_live_pr_state`.
- **Run creation lineage**: New runs automatically populate parent links and stack position from previous runs in the same wave.
- **UI tweaks**: Sidebar section renamed "On Disk" → "Worktrees"; `WorktreeRow` prefers `shortName` over `branch` for display.

## Key choices

1. **On-demand sync, not periodic**: Live PR state is fetched during wave DTO construction. No background polling yet — deferred until stale rates justify it.
2. **Shared projection logic**: `build_wave_live_pr_projection` centralizes the sync-fetch-store-project pipeline. Both wave DTOs and run list endpoints use it.
3. **Stale marker over fabrication**: When GitHub is unreachable or token is missing, PRs are marked stale rather than assumed open. This prevents phantom open counts.
4. **Backfill is best-effort**: Migration infers lineage from existing data using `LAG()` window function. Inferred rows are flagged with `lineage_inferred = true`.
5. **Default implementations in trait**: `find_next_unmerged_run` and `find_descendants` have default implementations on `RunStore` that compose existing methods, avoiding duplication across SQLite/Postgres backends.

## How it fits together

```
create_wave_run_with_id()
  → reads list_stack_runs() to find parent
  → populates lineage fields on new WaveRun
  → store.create_wave_run() persists them

build_wave_dto()
  → list_stack_runs() for the wave
  → build_wave_live_pr_projection() fetches/caches GitHub state
  → projects open_pr_count, stack_count, stale markers
  → wave_run_dto() attaches per-run live state
```

The live PR sync happens at read time (wave DTO construction), not at write time. This keeps the write path fast and makes sync failures non-fatal.

## Risks and bottlenecks

- **GitHub API rate limits**: Every wave DTO construction fetches PR state for all stack runs. With many PRs or frequent polling, this could hit rate limits. Mitigation: the `live_pr_states` table caches results, and stale markers degrade gracefully.
- **N+1 fetches**: Each PR triggers a separate GitHub API call. Batch fetching would be more efficient but isn't available in the REST API without GraphQL.
- **Sync at read time**: Wave list endpoints trigger live sync for every wave. For dashboards showing many waves, this adds latency. Consider adding a TTL cache or background sync if this becomes a problem.

## What's not included

- Draft/Ready promotion logic (step 02).
- Merge-triggered rebase and queue advancement (step 02).
- Combine PR reconciliation (step 03).
- Queue-first Concerto UX (step 04).
- Periodic background sync — deferred until stale rates are measurable.

## Bug fixed during gate

- `PullRequestResponse.is_draft` was mapped from `isDraft` (camelCase) but the GitHub REST API uses `draft` as the field name. The `#[serde(default)]` masked this — `is_draft` was always `false`. Fixed to use `draft` field name.
