# A claimed CI wake preempts an interactive review wait

## Problem

A Task parked in an interactive review never repairs its own red PR.

Measured twice on current release-provenance main — W2-290/#1041 and
W2-306/#1059 — each produced a `human_assisted=false` CI incident and an
attributable `CiFix` command claimed by the already-running Task generation. The
command then sat `Claimed` with `responded_at` null, forever.

The cause is one deliberate branch. W2-303 taught the runner that CI must not
interrupt an ordinary active provider turn, so the command poll defers `CiFix`
while `provider_turn_active` (`task/runner.rs:361-376`): it claims the command,
drops it from `seen_commands` so a later tick re-sees it, and retains it out of
`absorb_commands`. The arm rides the next `TurnCompleted` boundary instead
(`runner.rs:487`). That is right for ordinary work — a turn that is doing
something ends, and the repair takes the boundary it releases.

An interactive review is not ordinary work. Its turn is the agent *waiting*:
`human_interaction_review_protocol` (`runner.rs:1356`) literally instructs
"Ask bounded questions and wait for their FIFO follow-up messages." Nothing in
that turn is racing toward a boundary. The repair waits on a turn that is itself
waiting on a human, and the wake is never serviced.

Who benefits: every Task whose PR goes red while a review is open — which is the
common case, because review is exactly when a PR is finished enough to have CI
running against it. This serves Developer Efficiency's KR "No Task strands on a
dead body": a body that holds a claimed command it can never service is stranded
while still breathing.

### Which review waits are actually parked

Worth stating precisely, because the directive's phrasing ("interaction_review is
Some") is broader than the defect and I want the guard's cost understood. A
*freshly opened* Project-policy review sets `provider_turn_active = false`
(`start_prepared_task_step`, `runner.rs:1452-1469` — only a `Human` reviewer gets
`apply_input`). Those already arm through the existing idle poll at
`runner.rs:377`; that path is not broken and this change does not touch it.

The turns that park are:

| Shape | Where | Why it holds |
|---|---|---|
| Human review, fresh | `runner.rs:231-236` | protocol says wait for human messages |
| Any review, resumed at boot | `runner.rs:238-256` | `review_start`/`review_recovery` → `apply_next_pending` → active turn |
| Any review, reviewer follow-up | `runner.rs:419-427`, `612-621` | the answer turn can end in another wait |

All three are `provider_turn_active && interaction_review.is_some()`. One guard
covers them, and on the non-parked shapes the guard is unreachable — the branch
it lives in only runs while a turn is active.

## The demo

A Task sits in an open review with a red PR. Today `lf task status` shows the
`CiFix` command `Claimed` and the incident `responded_at` null, indefinitely.
After this change the same state resolves itself: the review turn is interrupted
once, the bounded ci-fix flow starts in that same generation on that same
command, the incident stamps `responded_at`, and the review record is still
`Active` for the next generation to resume. The runnable form is
`a_parked_review_wait_is_preempted_once_by_a_current_wake` — it hangs to the test
timeout on today's code and passes in ~1s with the fix.

## Approach

Add the missing *release* for a boundary that will otherwise never come. Do not
add a repair path.

In the `provider_turn_active` branch of the command poll: if a claimed `CiFix`
command names the PR's **current** failure, and an interaction review is open,
and we have not already done so for this turn — interrupt the harness once. The
resulting `TurnCompleted { status: Interrupted }` lands on the existing arm at
`runner.rs:487`, which claims and arms through `claim_and_arm_ci_fix`, starts the
bounded flow in the same generation, and clears the *local* `interaction_review`
while the durable record stays `Active`. Every one of those behaviors already
exists and is already tested; this change only makes the boundary arrive.

Three pieces:

1. **`current_ci_incident_identity(store, session) -> Result<Option<String>>`** —
   extracted verbatim from `arm_ci_fix_wake`'s existing currency read
   (`runner.rs:2392-2397`). One authority for "what is the PR failing *now*",
   read by both the arm and the preempt decision.

2. **`holds_current_ci_fix_wake(store, session, &[ChildCommand]) -> Result<bool>`** —
   the read-only half of the arm's selection. It answers "would preempting
   actually reach a repair" **without** superseding stale wakes or stamping
   `mark_ci_incident_responded`. Those are writes, they belong at the arm, and
   running them mid-turn is exactly what the deferral exists to prevent.

3. **`review_preempted: bool`** — one bit, set at the interrupt, cleared where
   `provider_turn_active = false` at `TurnCompleted` (`runner.rs:469`). Scope is
   one interrupt per provider turn.

The poll branch's existing loop-then-`retain` collapses into one `partition`, so
the net patch is around 25 lines.

```rust
let (wake, commands) = if provider_turn_active {
    let claimed = claim_commands(&store, &session, lease, &mut seen_commands).await?;
    let (ci_fix, rest): (Vec<_>, Vec<_>) = claimed
        .into_iter()
        .partition(|command| matches!(&command.kind, ChildCommandKind::CiFix { .. }));
    for command in &ci_fix {
        seen_commands.remove(&command.id);
    }
    // An interactive review turn is not ordinary work: it is the agent
    // waiting, so no boundary is coming to release the repair. Take it once.
    // The durable review stays open and the arm rides the resulting
    // `TurnCompleted` like any other.
    if !review_preempted
        && interaction_review.is_some()
        && !ci_fix.is_empty()
        && holds_current_ci_fix_wake(&store, &session, &ci_fix).await?
    {
        harness.interrupt().await?;
        review_preempted = true;
    }
    (None, rest)
} else if ci_fix_wake.is_none() {
    claim_and_arm_ci_fix(&store, &session, lease, &mut seen_commands).await?
} else {
    (None, claim_commands(&store, &session, lease, &mut seen_commands).await?)
};
```

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is the review turn actually an active provider turn? | Yes, for the three shapes tabled above. A fresh Project review is not (`runner.rs:1452-1469`) — it idles and already arms. | Guard is `provider_turn_active && interaction_review.is_some()`; the fresh-Project path is untouched. |
| Can I reuse `arm_ci_fix_wake` to test currency? | No. It supersedes stale wakes and stamps `mark_ci_incident_responded` (`runner.rs:2415-2468`). Calling it mid-turn arms a repair the body cannot start. | Split the pure read out; keep every write at the arm. |
| Does an interrupted review turn reach the arm? | Yes. `TurnCompleted` sets `provider_turn_active = false`, and the `ci_fix_wake.is_none()` arm at `runner.rs:487` runs before `resume_interrupted_flow`, which is `flow_turn_active && ...` and `flow_turn_active` is false under a review (`runner.rs:703`). | No new arming path. |
| What if the interrupt lands but the wake goes stale in the window? | Benign. Nothing arms; `resume_interrupted_flow` is false; control reaches `runner.rs:729` — `if interaction_review.is_some() { continue 'runner }`. The body idles with the review open, and a reviewer follow-up restarts the turn through `apply_next_pending`. | No compensating logic needed. The currency check makes this rare, not impossible, and the fallback is the pre-existing idle state. |
| Does the durable review survive the repair? | Yes. `interaction_review = None` at `runner.rs:508` is a local variable. The ci-fix body exits via `settle_ci_fix_turn` (`runner.rs:829`), and the next generation reopens the `Active` record at `runner.rs:228-256`. | Nothing to add; assert it in the test. |
| Does this break W2-303's rule? | No. `a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle` has no interaction review, so the guard is false and it must stay at `interrupts == 0`. That test is the regression. | Keep it green; treat a red there as this change being wrong. |
| Should a failed interrupt fail the body? | Yes, propagate with `?`. It matches the control path (`runner.rs:3876-3889` expects interrupt failure to fail control). Swallowing it would set no bit and re-interrupt every 200ms against a harness that cannot be interrupted. | `harness.interrupt().await?`. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Arm ci-fix directly during the review turn, no interrupt | Skips a boundary | The provider turn is still live; the ci-fix seed would race the review's transcript. This is the "do not add a parallel repair path" the directive forbids, and it is what W2-303 removed. |
| Deliver the wake as provider text into the review turn | No interrupt, no new state | Turns a durable command into prose. Nothing settles the command, `responded_at` stays null, and the ledger loses the repair. Explicitly forbidden. |
| Accept the command early, repair in a successor generation | No interrupt | Accepting before a repair is a lie the ledger cannot walk back, and it spends the incident identity — `ensure_child_ci_fix_command` would never relaunch. Forbidden, and it recreates W2-290. |
| Complete/close the review, repair, reopen | Clean state machine | A `Completed` review is write-once (`_complete_interaction_review`) and a wrong disposition is terminal. Never dispose a review to unblock plumbing. |
| Time-bound the review wait (interrupt after N minutes) | No CI coupling | A timer that guesses. The wake is the signal; a review with no red PR should wait as long as it likes. |
| Preempt on *any* claimed wake, skip the currency read | Smaller patch | Interrupts a review for a wake that `arm_ci_fix_wake` would then supersede as stale — a review turn destroyed for no repair. The currency read is the whole justification for the interrupt. |

## Key decisions

**The guard is `interaction_review.is_some()`, not "the reviewer is human."** A
resumed Project review parks identically (`runner.rs:249-256`). Keying on reviewer
kind would fix W2-290 and leave W2-306 — the two measured incidents — split.

**Currency is checked before interrupting, not after.** Interrupting first and
letting the arm decide would spend a real review turn on a stale wake. Reading
first costs one `active_task_pr` per tick while a review is parked with a claimed
wake — a rare, already-degenerate state.

**One bit over a new type.** `review_preempted: bool` next to
`turn_had_durable_side_effect`, cleared at the same line that clears
`provider_turn_active`. Per-turn scope falls out of where it is cleared, so the
degenerate loop (interrupt → no arm → review resumes → interrupt again) cannot
run: after `TurnCompleted` either the wake armed (`interaction_review` is `None`)
or it went stale (`holds_current_ci_fix_wake` is false). Both close the guard.

**The pure/impure split is the load-bearing part.** The reason this defect was
one branch and not a design flaw is that "is there a current wake" and "arm the
current wake" were the same function, so the poll could only ask the question by
committing to the answer. Extracting the read is what makes the preempt decidable
mid-turn without touching the ledger.

## Scope

- In scope: the preempt branch in the command poll; the two extracted read
  helpers; the `review_preempted` bit; the tests below.
- Out of scope: the fresh-Project-review idle path (already arms); ci-fix flow
  content, seeding, bounds, or settlement; review policy, disposition, or the
  human-review gate; the wake enqueue/observer side; W2-303's ordinary-work rule,
  which this preserves.

## Done when

`cargo test -p loopflow --lib task::runner`, plus `cargo fmt` and
`cargo clippy -p loopflow --lib --tests -- -D warnings` (the `--tests` target
matters: a lib-only clippy does not lint in-file test modules — W2-300 lost a CI
run to exactly that).

New in `task/runner/ci_fix_lifecycle_tests.rs`:

- **`a_parked_review_wait_is_preempted_once_by_a_current_wake`** — a harness whose
  first `send_input` opens a review turn, queues a ci-fix command, and **never**
  sends `TurnCompleted`; its `interrupt()` counts, then emits
  `TurnCompleted { Interrupted }` after ~500ms so several 200ms poll ticks pass
  while the turn is still active and already preempted. Asserts: `interrupts == 1`
  (once, across ticks), `sends == 2` (the ci-fix seed follows), the settled
  command is the same id claimed by the same generation, `responded_at` is
  stamped, and the durable review row is still `Active`.
- **`a_stale_wake_never_preempts_a_review_wait`** — same parked review, but the PR
  moves to a head whose current incident identity differs from the claimed wake.
  Asserts `interrupts == 0` and the review turn is untouched.

Existing, must stay green:

- **`a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle`** — no review
  open, so `interrupts == 0`: ordinary active work is never interrupted.
- **`an_interrupted_turn_leaves_the_wake_claimed`**, **`a_crash_after_arm_reclaims_the_same_command_and_reselects_ci_fix`** — recovery is unchanged.

Sabotage proofs (run each, confirm red — a test that passes with the fix removed
is pinning the fixture, per MEMORY's "sabotage the code the test names"):

| Remove | Must turn red |
|---|---|
| the whole preempt block | `a_parked_review_wait_is_preempted_once_by_a_current_wake` (hangs to timeout) |
| `interaction_review.is_some()` | `a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle` |
| `!review_preempted` | `a_parked_review_wait_...` on `interrupts == 1` |
| `holds_current_ci_fix_wake(..)` | `a_stale_wake_never_preempts_a_review_wait` |

The last row is why the stale test exists: with only a current-wake test, dropping
the currency check turns nothing red.

## Measure

Baseline, on the live store: `CiFix` commands sitting `Claimed` against a live
generation whose incident has `responded_at` null — W2-290/#1041 and W2-306/#1059
are the two known, both permanent. After: every such command reaches a terminal
state (`Accepted`/`Failed`/`Superseded`) within one review turn's interrupt, and
`responded_at` stamps at the arm. The honest measure is not a fleet count — per
MEMORY's ENG-4 correction, nothing sweeps the table, so a snapshot always catches
transients. It is that **no claimed wake is permanent**: each one is armable at
its next poll tick.
