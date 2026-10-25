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

**Release run.** `wait_for_pr_merge` (`release.rs`) asks
`repair_running(store, repo, pr)` when the PR has no merge request; while a
repair is live it keeps polling without reading removals or re-arming the old
head. `landing_repair_processes(landing_id)` became `pr_repair_processes(repo, pr)`,
and `repair_running` takes `(repo, pr)` too, so any landing's repair on the PR counts.
Test: `release_run_waits_out_a_repair_disarm_but_stops_on_a_persons`.

**Deferred (no Task yet).** Have the block message say a repair ended without
re-arming. Check whether exact-head arming makes the repair's disarm
unnecessary. Both are in `loo-577-5whys.md`.

**Status (2026-10-24).** Waiter and release-run fixes implemented and gated; landing.
Fixture process ids are now UUIDs (`LfProcessId` became UUID-only on main).
Check: `cargo test -p loopflow --lib repair_disarm_keeps_landing` + `--test release_tests release_run_waits_out_a_repair_disarm` → pass; clippy deferred to gate.
Gate: `cargo fmt --check` + `cargo clippy --all-targets -D warnings` → pass; `--lib pr_landing` (20), `--lib ci_watch` (9), `--test release_tests` (75) → pass. New release test takes 27s on the existing 10s merge poll.

**Concept review (2026-10-24).** One rule in both waiters: a live repair on the PR
owns its merge request, so its disarm is a pause. It holds because `run_repair`
calls `finish_repair` only after `process_ci_fix` returns, past the re-arm. A
repair that ends blocked without re-arming still reads as a dequeue, as the plan says.
No design change and no open product choice.
