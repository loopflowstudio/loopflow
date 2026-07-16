# W2-156 — the ci-fix wake (directive v4: narrow Product slice, dependent on Infra)

Directive v4 split this Task. The four hardest done-whens are now owned by
Infrastructure tasks and must NOT be reimplemented here:

- **W2-229** — deterministic failed-PR ci-fix lifecycle proof.
- **W2-230** — durable audited ci-fix command ledger (`ChildCommandKind::CiFix`).
- **W2-231** — actionable `Blocked` on an agent-reported infrastructure failure.
- **W2-232** — bounded ci-fix settlement without generic-gate re-entry.

W2-156's remaining deliverable is the **incremental Product slice** (PR #967) plus
keeping the Task open and visibly dependent until those four merge.

## User-visible outcome

A Task sleeping on a *submitted* open PR whose current head fails a required
check is woken **once** into a bounded `ci-fix` turn that repairs the branch and
pushes the same PR, then sleeps on the new head. `lf status` reads `fixing CI`
while the turn runs. This is the narrow, dedup-bounded behavior — not yet the
full lifecycle contract (that needs W2-229–232).

## End-to-end proof (for this slice)

PR #967 green on all required leaves + `scratch-clear` + `tests-result`, landed
as a Product increment, with W2-156 still **open**. The narrow behavior is
proven by the merged unit tests:
- `ci_wake_dedup_fires_once_per_head_and_failure_set` (one wake per key).
- `merge_gate_seeds_actionable_leaves_not_the_required_aggregate` (leaf seeding,
  never `tests-result`).
- `ci_fix_restart_bar_permits_only_a_warranted_open_pr_wake`,
  `ci_fix_wake_refuses_an_open_pr_without_a_warranted_failure` (gate).
- `reconcile_process_liveness_consumes_queued_resume_before_settling` (W2-144
  queue bridge).
- `open_pr_next_move_is_ci_derived`, `open_pr_failing_with_live_generation_owns_task`
  (`lf status` owner).

The **full** proof (pending → wake → body → push → rearm → green; duplicate +
infra-blocked; audited command ledger) is W2-229's, not provable here.

## Source of truth

`CiObservation` on `TaskPr` (persisted): `head_sha`, `state`, `failing_checks`
(leaf names+URLs from `merge_gate_state`), `woken_failure_set` dedup key. The
wake is a direct supervisor launch (`inspect_outcome` → `wake_task_ci_fix` →
`LaunchIntent::CiFix`); the audited command ledger is W2-230's, deferred.

## Affected surfaces

`task/mod.rs` (dedup key), `ops/pr.rs` (`merge_gate_state` leaf expansion),
`ops/task.rs` (reconcile wake + queue bridge), `ops/child.rs`
(`LaunchIntent::CiFix`), `project_session/runner.rs` (supervisor trigger),
`task/runner.rs` (ci-fix turn seed), `lf status` owner, `ci-fix.yaml`.

## Absent / error states (this slice)

No PR / gh unavailable → no wake. Head moved → stale key dropped. Pending/green →
no body. Duplicate/multi-check → coalesced by `(head, failure_set)`. Infra
blocker → **currently returns to `Waiting`** (W2-231 will make it `Blocked`).
Completion → **may enter the generic gate cycle** (W2-232 will bound it). These
two are known-narrow, dedup-bounded and non-destructive.

## Operational boundary

One Task, one worktree, one provider history, one PR sequence. Wake ≤ one
generation per `(head, failure_set)` (`mark_woken` before any body); supervisor
cannot re-wake; gate cycles are counter-bounded. No second CI identity, no
monitor store.

## Exclusions (owned elsewhere / deferred)

W2-229 lifecycle proof; W2-230 audited command ledger; W2-231 actionable
Blocked; W2-232 bounded settlement; DTO `serde(default)` removal on
`woken_failure_set` (fold into W2-230's ledger work, not this slice). Webhook
receiver; reconcile cron; retry-cap records.

## Pursue target

1. Keep #967 green + rebased; land it as the increment with **bare**
   `lf pr land --next` semantics is wrong here — there is no next serial PR.
   Land keeps W2-156 open; do **not** `lf pr land -c` and do **not**
   `lf task complete`. A human may land, or hands-off land is acceptable given
   the dedup-bounded safety.
2. Confirm W2-156 stays open and its Linear description lists W2-229–232.
3. W2-156 completes only when those four merge and satisfy the original proof.
