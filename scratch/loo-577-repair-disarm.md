# LOO-577: lf's repair disarm read as a person's dequeue

**Symptom.** PR #1668 (LOO-573), 2026-10-25 02:24–02:54 UTC. A CI repair disarmed
auto-merge before pushing (`clear_task_pr_merge_before_head_mutation` →
`ops::pr::disable_auto_merge`). The Graph's `lf land --wait-and-fix` observed
`Unarmed` and blocked: "pull request has no auto-merge request; run lf arm to
resume landing". The repair re-armed at 02:54; the Workflow stayed stopped.

**Cause.** `reconcile_pr_landing` treated every `Unarmed` observation as revoked
intent, though the `Failing` arm already defers to a live repair (`repair_running`).
Repairs from the waiter and `lf ci watch` share one landing record
(`start_or_join_pr_landing`), so `repair_running` sees either.

**Repair.** `Unarmed` while a repair reservation on this landing is live → keep
watching. No new disarm record: the live reservation is the record. A disarm with
no live repair still blocks (existing `revoked_merge_intent_blocks_without_launching_a_repair`).

**Test.** `repair_disarm_keeps_landing_until_the_repair_rearms_and_merges`.

**Release run (in scope, next).** #1668 merged to main as `936afed62c`
(2026-10-25 03:13 UTC). `lf release run` calls
`ops::pr::manual_merge_removal_since(repo, pr, armed_at)` (`release.rs:3761`)
and turns any `manually_disabled` after its arm into `OpsError::ManualMergeRemoval`.
Apply the same rule there: if a repair reservation on the release PR's landing
was live, the disarm is not a dequeue. A disarm that no live repair accounts for
is still one. Steps: `lf sync` onto main, then implement and add a test where
the release run survives a repair's disarm window and still stops on a disarm
with no repair. If the release run has no landing record to check, let the
reader that finds the landing for a PR (`start_or_join_pr_landing`) answer
"repair live for PR N". Don't add a disarm table.

**Deferred (no Task yet).** Have the block message say a repair ended without
re-arming. Check whether exact-head arming makes the repair's disarm
unnecessary. Both are in `loo-577-5whys.md`.
