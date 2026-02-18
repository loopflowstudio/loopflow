# Wave Queue + Live PR State — Design Review

## What was implemented

Two features shipped as a single cohesive branch:

1. **Explicit run lineage**: wave runs track `parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`, and `stack_status`. This makes stacked wave relationships explicit instead of inferred from creation order.

2. **Queue lifecycle with deterministic advancement**: Draft-first PR creation, lazy rebase, single-Ready-PR enforcement, and merge-driven promotion. Merge events from both webhooks and polling advance the queue identically.

Supporting infrastructure:

- `live_pr.rs` module: shared `LivePrSnapshot` for batch-fetching and caching GitHub PR state, replacing ad-hoc per-function state loading.
- `queue.rs` module (814 lines): `reconcile_wave_queue`, `handle_pr_merged`, `project_queue_views`, per-wave reconcile locks.
- Two new tables: `wave_queue_blocks` (blocked reason/details), `wave_pr_merge_events` (merge-event deduplication).
- Queue projection fields on run DTOs: `queue_role`, `queue_block_reason`, `queue_blocked_at`, `next_action`.
- Typed `QueueBlockReason` enum replacing raw strings for block reasons.

## Key choices

**Queue role is projected, not stored.** `project_queue_views` derives `QueueRole` (Head / Waiting / Blocked / Merged) from stack order + live PR state + block records. No canonical queue-role column. This avoids state synchronization bugs at the cost of requiring the full stack for projection.

**Block facts are persisted, merge events are deduplicated.** `wave_queue_blocks` stores the reason and conflict files for blocked runs. `wave_pr_merge_events` ensures the same merge is processed exactly once across webhook and polling paths.

**`LivePrSnapshot` as shared infrastructure.** The previous approach had `load_live_states` and `live_for_run` duplicated between queue.rs and the HTTP routes. Extracting `live_pr.rs` with `build_live_pr_snapshot` gives a single fetch-and-cache path used by both queue reconciliation and API responses.

**Per-wave reconcile locks.** A global `Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>>` serializes concurrent reconciliation for the same wave while allowing different waves to reconcile in parallel. Prevents race conditions when webhook and poll trigger simultaneously.

**Typed `QueueBlockReason`.** Replaced raw `String` block reasons with a `QueueBlockReason` enum (`MissingPr`, `WaveRunning`, `ScratchDirty`, `RebaseConflict`, `PromotionFailed`). Database stores the string representation; parsing happens at read time.

**Draft-first PR creation.** New runs create PRs as Draft. Queue reconciliation promotes the head run to Ready only after rebase succeeds and scratch is clean. This enforces exactly one Ready PR per wave.

## How it fits together

```
run completion
    → auto_create_pr (Draft)
    → reconcile_wave_queue
        → build_live_pr_snapshot (fetch from GitHub, cache in DB)
        → infer stack statuses from live PR state
        → find queue head (first Active run)
        → check scratch clean, lazy rebase, mark Ready
        → clear blocks on success

merge (webhook or poll)
    → handle_pr_merged
        → record_merge_event (dedupe)
        → mark run stack_status = Merged
        → reconcile_wave_queue (promotes next run)
```

Queue projection (`project_queue_views`) runs at API response time, combining stack order, live snapshot, and block records into per-run `QueueRunView` DTOs.

## Risks and bottlenecks

- **Global static for reconcile locks.** The `QUEUE_RECONCILE_LOCKS` map grows unboundedly. In practice this is fine—each wave adds one entry (~100 bytes). If lfd manages thousands of waves, consider periodic cleanup.
- **GitHub API rate limits.** `build_live_pr_snapshot` fetches each PR individually. For waves with many stacked runs, this could hit rate limits. Batching via GraphQL would help but is out of scope.
- **Stale state in no-token environments.** When `github.token` is empty, all PR states are marked stale. Clients must treat `has_stale_pr_state: true` as expected degraded mode, not an error.
- **`QueueBlockReason` parsing.** If a future migration adds a new reason string before updating the binary, reads will fail with `InvalidData`. The `FromStr` impl returns an error for unknown values rather than silently degrading.

## What's not included

- Full descendant rebase cascades (only immediate next run is promoted)
- GitHub merge-queue / status-check integration
- Canonical historical queue-role persistence (no audit log)
- GraphQL batched PR fetching
- Queue advancement UI in Concerto
