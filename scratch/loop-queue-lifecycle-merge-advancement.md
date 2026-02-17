---
status: in_progress
seq: 2
source: wave/loop/02-queue-lifecycle-merge-advancement.md
---

# Queue Lifecycle + Merge Advancement

## Problem

Stacked wave PRs still behave like independent branches. In recurring waves, PRs can become Ready too early, merges done outside `lfd` are not reliably recognized, and the next branch is not deterministically rebased/promoted. Reviewers lose trust because “what should merge next” is ambiguous.

Who benefits:
- Wave operators: predictable one-at-a-time landing.
- Reviewers: only one mergeable PR at a time.
- Daemon operators: explicit blocked/stale states instead of silent drift.

Why now: step 01 made lineage + live PR state usable. Step 02 is where that data becomes queue automation.

## Approach

Build a wave-scoped **queue reconciler** that is the single authority for Draft/Ready/Blocked roles.

### 1) Introduce queue orchestration service

Add `lfd::queue` with one entrypoint:
- `reconcile_wave_queue(store, github, wave_id, trigger)`

Responsibilities:
1. Load stack runs oldest-first (`list_stack_runs`).
2. Refresh live PR state for candidate runs (token present) or mark stale.
3. Compute queue role per run (`ready|draft|blocked|merged|superseded`).
4. Enforce invariant: at most one Ready open PR.
5. Attempt lazy rebase/promote of the next candidate when queue-head merges.
6. Emit structured logs/metrics for each transition.

### 2) Keep queue role projected, persist only durable facts

- Keep `stack_status` as durable history (`active|merged|superseded`).
- Compute `queue_role` on read/reconcile (not stored as canonical state).
- Persist only blocked metadata in a new table (wave_id + run_id + reason + attempted_at + conflict_files + error).
- Persist merge-event dedupe entries (wave_id + pr_number + merged_at + processed_at).

This avoids stale stored queue-role drift while preserving recoverable blocked state.

### 3) Make PR creation always Draft, then reconcile

On run completion:
1. Create/update PR as Draft.
2. Sync live PR state.
3. Call `reconcile_wave_queue`.

Change current recurring-wave behavior so it does **not** auto-mark ready at creation time. Queue promotion now flows through one reconciler path.

### 4) Detect merges from webhook + poll using same handler

Add merge input paths that both call `handle_pr_merged(wave_id, pr_number, merged_at)`:
- GitHub webhook: `pull_request` with `action=closed` and `merged=true`.
- Poll fallback: periodic scan of open stack PRs, detect transition to `LivePrState::Merged`.

Handler is idempotent via dedupe table/lock.

### 5) Lazy rebase only immediate next candidate

After a queue-head merge:
1. Mark merged run as `stack_status=merged`.
2. Pick next open Draft whose ancestors are merged/superseded.
3. Rebase only that branch onto `origin/main`.
4. Push `--force-with-lease`.
5. Promote Draft -> Ready.

On conflict: keep Draft, mark `blocked` with conflict details, stop deeper promotions.

### 6) Promotion gates

Before Draft -> Ready:
- PR exists and is open.
- No wave advancement lock held.
- `scratch/` tracked diff is clean.

Gate failure leaves Draft and records actionable block reason (`scratch_dirty`, `missing_pr`, `rebase_conflict`, etc.).

### 7) API + observability

Extend run DTO projection:
- `queue_role`
- `queue_block_reason`
- `queue_blocked_at`
- `next_action` (`open_pr|resolve_conflict|combine_prs|await_merge`)

Add counters:
- merge events received/processed/deduped
- promotion attempts/success/failure
- rebase success/conflict
- blocked queue count
- promotion latency (merged_at -> next ready)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Rebase full descendant chain after every merge | Keeps all descendants current, but expensive and conflict-prone | Violates locked v1 scope; too much churn and failure surface |
| Use GitHub merge queue as primary source of truth | Less local logic, but loses wave lineage/control and local-daemon parity | Conflicts with loopflow’s explicit stack and combine modeling |
| Manual `lf ops` promotion only | Lowest implementation cost | Fails core goal: deterministic auto-advancement from any merge surface |

## Key decisions

1. **Queue reconciler is the only mutation path.** All creation/merge/poll/webhook triggers funnel through it for deterministic behavior.
2. **Project queue role; persist block facts only.** Prevents role drift while keeping debuggable recovery context.
3. **No “smart cascade” in v1.** Follow wave principle: **“Rebase only the immediate next draft after each merge (lazy rebase).”**
4. **Draft-first is strict.** Follow wave principle: **“Create PRs as Draft by default.”**
5. **Single ready is strict.** Follow wave principle: **“Keep exactly one Ready PR (oldest unmerged) at any time.”**
6. **Cross-surface merge handling is mandatory.** Follow wave principle: **“Detect merges from any surface … and advance queue automatically.”**

Wild success signal: operators stop thinking about queue mechanics; they only resolve explicit blocks.

Wild failure to avoid: hidden auto-promotion races create multiple Ready PRs and force manual DB repair.

## Scope

- In scope:
  - Queue reconciler service and idempotent merge handler
  - Draft-first PR creation path update
  - Webhook + polling merge detection convergence
  - Lazy rebase + blocked-state persistence
  - Queue projection fields on wave run APIs
  - Queue advancement tests + conflict/scratch gate tests
- Out of scope:
  - Full descendant rebase cascade
  - GitHub status-check/merge-queue integration
  - Combine PR reconciliation modeling (step 03)
  - Concerto redesign (step 04)

## Done when

- `cargo test --all` passes with new queue lifecycle coverage, including:
  - only one Ready PR exists for a wave at any point
  - queue-head merge promotes exactly one next run
  - duplicate merge events are idempotent
  - rebase conflict marks blocked and halts deeper promotion
  - clearing block + resync resumes promotion
  - scratch diff blocks promotion with `next_action=resolve_conflict`
- `GET /wave_runs?wave_id=<id>&order=stack` returns queue fields (`queue_role`, block metadata, `next_action`) with oldest-first stability.
- Merging the Ready PR from GitHub UI or poll-detected path advances queue identically within one reconciliation cycle.
