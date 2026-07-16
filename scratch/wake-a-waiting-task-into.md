# W2-156 PR2 — the ci-fix wake (design re-established after PR1 land)

PR1 (#916, merged 2026-07-15) shipped *truthful CI observation*: `TaskPr` now
persists `github_head_sha` + a `CiObservation{head_sha,state,failing_checks,
observed_at}` (migration `0.11.004_task_pr_ci_state`), `reconcile_task_pr` reads
`gh pr checks --required` for the open head, `TaskPr::fresh_ci()` gates the
reading on the current head, and `lf status` routes an open PR to the
`NextMoveOwner::Ci` producer (pending/failing → Ci, passing → Review). The
design doc died at land; this note re-establishes PR2 from wave memory.

## User-visible outcome

A Task waiting on an open PR whose required check **fails** on the current head
is woken by Loopflow into one bounded `ci-fix` turn that repairs the branch,
pushes the same PR, and returns to sleep watching the new head — no human
notices the red X or rescues the worker. `lf status` reads `fixing CI` while the
turn runs, `blocked` if it reports an infra blocker, back to `awaiting review`
when green.

## Source of truth

Extend PR1's `CiObservation` on `TaskPr` with the **dedup key**: the
`(head_sha, failing_check_set)` a wake has already fired for. A wake is enqueued
only when the current fresh reading is `Failing` and its `(head, failure_set)`
differs from the last-woken key. No new store, no CI monitor table.

## Mechanism

1. **Wake command.** New `ChildCommandKind::CiFix { pr_number, url, branch,
   head_sha, failing_checks:[{name,url}] }` enqueued on the existing child-command
   path, once per `(head_sha, failure_set)`. Reconcile stamps the dedup key when it
   enqueues, so a repeated poll/webhook can't create a second command or body.

2. **`LaunchIntent::CiFix`** (`ops/child.rs`) — a third intent past
   `supervisor_restart_bar`'s open-PR bar, allowed **only** with a `CiFix` command
   carrying fresh current-head failure evidence. Not a blind supervisor restart
   (never re-opens `task_clarify` over submitted work — the W2-129 bar stays), not
   an operator resume. The lease CAS (`reserve_task_process`) keeps exactly one
   body: a second wake while a ci-fix generation is live can't reserve.

3. **The ci-fix turn.** The Task runner (`task/runner.rs`) currently hardcodes the
   `task` flow. When a `CiFix` command drains at startup, load a new single-step
   builtin flow `engine/builtins/build/flow/ci-fix.yaml` (step: the existing
   `ci-fix` skill, which already consumes injected `{PR#, branch, head_sha, logs}`)
   seeded from the command metadata, instead of `task.yaml`. After the body pushes
   (head SHA advances), reconcile, return to `Waiting`, rearm on the new head. A
   new failure at the new head is a new key → one new turn. Infra/credential
   blocker → body makes no worktree change → settles `Blocked` (existing
   "completed without a PR or worktree change" path) with the blocker reason
   visible; tests never weakened.

4. **Manual resume dominates (W2-144 gen 7).** Bridge the out-of-loop
   `reconcile_process_liveness` (`ops/task.rs`) settle: before setting `Waiting` on
   a dead-process open-PR Task, consult the command queue — a queued `Resume`
   (or `CiFix`) relaunches instead of settling, so a manual `lf task resume` is
   consumed once, never discarded because the PR is open. One relaunch path, two
   producers.

## Gate the wake on real, post-submit failures (PR1 dogfood, PR #916)

`tests-result` is a CI **aggregate** that goes red when any leaf check
(including `scratch-clear`) is red — it does not mean a test failed. A PR opened
via `lf pr open` always fails `scratch-clear`+`tests-result` until submit/land
clears `scratch/`. Therefore:
- The waiting-on-open-PR state that arms the wake must be reached via
  `lf pr submit`/`lf pr land` (scratch cleared), so `scratch-clear` never
  spuriously wakes a ci-fix turn.
- The wake's `failing_checks` must carry the **leaf** check names, never the
  `tests-result` aggregate. PR1's reconcile already reads leaf checks via
  `gh pr checks --required` (aggregate and leaves are separate entries), so this
  holds by construction — assert it in a PR2 test.

## Affected surfaces

`task/mod.rs` (dedup key on `CiObservation`; `ChildCommandKind::CiFix`),
`ops/child.rs` (`LaunchIntent::CiFix`), `ops/task.rs` (reconcile wake decision +
dedup stamp; `reconcile_process_liveness` queue bridge), `task/runner.rs` (CiFix
turn + rearm; infra-blocker reason), new `ci-fix.yaml`, `lf status` "fixing CI"
owner refinement (failing + ci-fix generation live → Task).

## Absent / error states

No PR / gh unavailable → no wake. Head moved → per-head dedup drops the stale
key; stale failures never wake. Pending → healthy wait, no body. Duplicate /
multi-check delivery → coalesced by `(head, failure_set)`. Infra blocker →
`Blocked` with actionable reason, zero churn.

## Operational boundary

One Task, one worktree, one provider history, one PR sequence. No second CI
worker identity, no duplicate Task/worktree/transcript, no CI monitor store. CI
evidence authoritative only for the current head. Wake rides the existing
child-command path; execution reuses the lease/generation machinery.

## End-to-end proof

Deterministic integration harness (mock gh check reads, in-memory store,
scripted harness like `task/runner.rs` tests): pending → no body; one required
failure → exactly one `CiFix` command + one generation + a ci-fix body seeded
with canonical metadata; duplicate + multi-check → one wake; push advances head
→ return to Waiting, rearm; green → no body; infra failure → Blocked with
blocker, no churn; W2-144 gen 7 → queued manual resume consumed once, not
discarded. Assert `failing_checks` carries leaf names, never `tests-result`.

## Exclusions

Webhook receiver (poll/reconcile is baseline; dedup makes a webhook safe later);
bounded-latency reconcile cron over unsupervised waiting Tasks; retry-cap /
non-convergence records beyond the failure-set dedup; non-required checks never
wake.

## Status (2026-07-15)

- **Slice 1 shipped** (`529b19888`): the `(head, failure_set)` dedup key on
  `CiObservation` (`woken_failure_set` + `wake_warranted()` + `mark_woken()` +
  reconcile carry-forward), tested.
- **Slice 2a shipped** (`eb8a20a27`): `TaskSession::ci_fix_restart_bar` — the
  launch gate that permits an open-PR restart only on a `wake_warranted()`
  current-head failure, leaving the W2-129 supervisor bar intact elsewhere.
- **Slice 2b shipped** (`7605bdbb5`): `LaunchIntent::CiFix` + `wake_task_ci_fix`
  (shared child-launch entry past the bar) + the supervisor trigger — project
  `inspect_outcome` wakes a sleeping open-PR Task when its fresh reading is
  `wake_warranted()`. Tested (a green open PR refuses the wake).
- **Slice 2c shipped** (`de49527d5`): the wake is now functional end to end. At
  generation startup `arm_ci_fix_wake` detects a `wake_warranted()` failure on the
  active PR's head, marks the observation woken + persists (idempotent), and the
  runner loads the single-step builtin `ci-fix.yaml` (registered) instead of
  `task.yaml`; `prepare_task_flow_step` seeds the ci-fix step with the PR + failing
  leaf checks + log URLs. After the push, the existing settle path returns the Task
  to `Waiting` and reconcile rearms on the new head. Tested.
- **Slice 3 shipped** (this rebase): completes PR2.
  - **3a — `reconcile_process_liveness` queue bridge (W2-144 gen 7):** before
    settling a dead-process open-PR Task to `Waiting`, the reconcile path now
    checks the command queue for a pending `Resume`. If found, it relaunches
    (consuming the Resume via the new generation's command drain) instead of
    settling — so a manual `lf task resume` is never discarded because the PR is
    open. One relaunch path, shared with the ci-fix wake. Tested.
  - **3b — `lf status` "fixing CI" owner:** `next_move_for_task` now returns
    `Task` (not `Ci`) when an open PR has a failing fresh reading AND the Task
    status is `Running`/`Starting` — the ci-fix turn is live, the Task owns the
    next move. Idle (`Waiting`) + failing → `Ci` (the wake will fire). Passing →
    `Review` regardless. Tested.
  - **3c — integration harness:** deterministic test proving the queue bridge
    (with-Resume relaunches, without-Resume settles) + the owner refinement
    (Running+failing→Task, Waiting+failing→Ci, Running+passing→Review). The
    pre-existing `ci_fix_seed` and `ci_fix_restart_bar` tests cover slices 1–2c.
  - **Pre-existing fix:** `task/runner.rs` test `TaskPr` initializer was missing
    `parent_pr_id` (surfaced by the rebase); fixed.
- **Slice 4 shipped** (reconcile of #967): the wake now seeds **actionable leaf
  checks**, never the required aggregate. Verified against live CI: this repo's
  *only* required check is the `tests-result` roll-up, so PR1's `gh pr checks
  --required` read carried only `tests-result` — an aggregate whose job link
  points at the aggregation step, not the broken job. The design assumed
  `--required` returned leaves ("aggregate and leaves are separate entries");
  live CI proved the opposite. Fix: `ops/pr.rs::merge_gate_state` reads
  `--required` for the *gate* state (failing/pending/passing — the merge gate is
  still authoritative) and the full check set for the *leaves*; `failing_leaves`
  drops the required aggregates when a non-required leaf also failed, keeps a
  required check that is itself the only failing leaf, and degrades to the
  required checks if the full read is empty. `observe_required_checks` seeds
  `CiObservation.failing_checks` from the leaves. This makes the design's
  done-when true by construction (leaf names, never `tests-result`) and tightens
  the `(head, failure_set)` dedup key onto the real failing jobs. Tested
  (`merge_gate_seeds_actionable_leaves_not_the_required_aggregate` +
  required-leaf + empty-full-read fallbacks).
</content>
