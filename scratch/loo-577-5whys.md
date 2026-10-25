# LOO-577 five whys: lf's own repair disarm stopped its own landing

**What failed.** PR #1668 (LOO-573), 2026-10-25 02:24–02:54 UTC. LOO-573's
autonomous Graph ran `lf land --wait-and-fix`; CI failed; a ci-fix repair disarmed
auto-merge before moving the head; the waiter saw no merge request and stopped
with "run lf arm to resume landing". The repair re-armed at 02:54, but the Graph
step had already exited held, so the Workflow stayed stopped on its edge.
Affected: Jack (had to notice and restart a landing nobody had withdrawn).

**Recovery status.** The PR re-armed itself; the stopped Workflow needs a person
or operator to run the edge again. Waiter fix is in this branch's diff
(`rust/loopflow/src/ops/pr_landing.rs`, `Unarmed` arm), unmerged. Release-run
half is not fixed (its code lives on unmerged #1668).

## Causal chain

1. **Why did the waiter stop?** `reconcile_claimed` mapped every
   `LandingObservation::Unarmed` to `block_landing` → `OpsError::DeliveryHeld`,
   which ends `lf land` and the Graph step. Evidence: `pr_landing.rs` `Unarmed`
   arm before this diff.

2. **Why was the PR unarmed while landing was live?** The repair's first head
   mutation (`lf sync`/`lf commit` inside the ci-fix agent) calls
   `clear_task_pr_merge_before_head_mutation` → `ops::pr::disable_auto_merge`
   (`task.rs:2155`). The ci-fix prompt then tells the agent to `lf arm`
   (`pr_landing.rs` repair prompt). So disarm→re-arm is a designed window of
   tens of minutes, not a fault.

3. **Why could the waiter observe that window?** `admit_ci_fix` spawns a detached
   `lf task __repair` worker and returns as soon as the worker claims the
   reservation. The waiter keeps polling while the worker runs, so it sees every
   intermediate GitHub state. The `Failing` arm already knew this
   ("Publication can expose a new head before its repair worker exits") and
   defers to `repair_running`; `Unarmed` did not.

4. **Why did nobody notice the asymmetry?** The waited-landing work (LOO-503)
   modeled one transient — a new failing head mid-repair — and tests cover
   Failing-during-repair. The disarm is a side effect of a different module
   (Task PR mutation in `task.rs`), so its effect on the landing state machine
   isn't visible where `Unarmed` is classified. Test `revoked_merge_intent_blocks_without_launching_a_repair`
   pinned "Unarmed ⇒ block" without a repair-live case.

5. **Why can't GitHub evidence tell lf's disarm from a person's?** lf disarms
   with the person's own `gh` credential; GitHub records `AutoMergeDisabledEvent`,
   `reasonCode: manually_disabled`, actor = Jack. Any reader that infers intent
   from provider history (the waiter here, `lf release run` on #1668) inherits
   this ambiguity. Only Loopflow's own records know which disarms it made.

Branch — **why did the stop strand the Workflow?** A held landing has no
resumption path: once the step exits, a later re-arm by the same repair is
observed by nobody. That converts any false hold into a person's chore. Not
changed here; noted below.

## Fix made (this branch)

`Unarmed` while `repair_running` → `resume_watching`, same rule as
`Failing`. The live repair reservation is the record of "lf disarmed this"; no new
disarm table. Repairs from the waiter and `lf ci watch` share one landing
(`start_or_join_pr_landing`), so either is seen. A disarm with no live repair still
blocks. Test: `repair_disarm_keeps_landing_until_the_repair_rearms_and_merges`.

Residual: if the repair dies after disarming and never re-arms, the waiter
blocks with "run lf arm" — truthful and actionable, though the message says
nothing about the dead repair.

## Proposed (not done)

- **Release run (#1668).** Its "`manually_disabled` after latest arm = dequeue"
  rule needs the same exemption: not a dequeue while `repair_running`. Land with
  #1668 or immediately after; test: release run survives a repair disarm window.
- **Block message names the repair.** When `Unarmed` follows a finished/failed
  repair reservation, say "ci-fix repair ended without re-arming: <conclusion>"
  instead of implying a person disarmed. Cheap; improves 2 a.m. logs.
- **Question the disarm itself.** If arms use `matchHeadCommit`-style exact-head
  intent, a pushed head already invalidates the old arm and the explicit disarm
  may be redundant for repairs. Worth checking before adding more readers that
  must special-case it — deleting the window deletes the failure class.

## Open questions

- Was #1668's repair launched by this waiter or by `lf ci watch`? Either path
  reserves on the same landing, so the fix holds; logs weren't checked.
- Does any other reader (Desktop status, `lf task status`) present a
  repair-window `Unarmed` as "not landing"? Not surveyed.
