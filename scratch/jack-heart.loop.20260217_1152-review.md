# Queue Lifecycle + Live PR State Review

## What was implemented

- Added wave queue orchestration (`lfd::queue`) with `reconcile_wave_queue` and idempotent `handle_pr_merged`.
- Added durable queue persistence:
  - `wave_queue_blocks` for actionable blocked reasons
  - `wave_pr_merge_events` for merge-event dedupe
- Added live PR-state synchronization and projection updates so wave/run APIs use live state as source of truth for `open_pr_count` and stale-state signaling.
- Extended wave run DTOs with queue-facing fields: `queue_role`, `queue_block_reason`, `queue_blocked_at`, `next_action`.
- Unified merge advancement paths:
  - webhook `pull_request` merged handling
  - polling trigger path via queue reconciler
- Enforced draft-first PR behavior on run completion and queue-head promotion through reconciliation.

## Key choices

- Queue role stays projected (not canonically stored), while block facts and merge dedupe are persisted.
- Reconcile logic promotes only the immediate next eligible run (lazy rebase), not full-descendant cascades.
- Live PR state remains authoritative for current open/closed/merged behavior; snapshot PR data remains historical.
- Run-completion reconciliation now uses the configured GitHub settings (`WaveExecutor` threads real `GitHubConfig` instead of `GitHubConfig::default()`).
- Added a guardrail test that locks `WaveRunKind::Main` storage value to `1` to protect migration assumptions.

## How it fits together

A completed run always creates/updates a Draft PR, upserts local live PR state, then calls queue reconciliation. Reconciliation refreshes live PR state (when token is available), computes queue head from oldest active stack run, applies promotion gates (missing PR, running wave, scratch dirty, rebase conflict), and promotes exactly one head to Ready. Webhook/poll merge detection calls `handle_pr_merged`, which dedupes merge events and re-runs reconciliation.

## Risks and bottlenecks

- Queue role is computed, not historical; consumers needing immutable timeline views still need event/audit modeling.
- Reconciliation depends on GitHub API freshness when token is present; without token, stale flags are expected and must be handled by clients.
- Branch rename in concurrent test conditions could race on `.git/config`; retry logic was added to reduce transient lock failures.

## What's not included

- No full descendant rebase cascade after merge (lazy immediate-next only).
- No GitHub merge queue/status-check gate integration.
- No persisted canonical queue-role history model.
