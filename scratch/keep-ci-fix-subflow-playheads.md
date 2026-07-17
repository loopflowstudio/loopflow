# Keep ci-fix subflow playheads out of the durable Task cursor

## Problem

A `ci-fix` body is a bounded repair turn that borrows a live Task Session's
generation. It runs a *different flow* than the Task's lifecycle phase: the
transient playhead reads `ci-fix` while the durable cursor reads `task-kickoff`
(Kickoff), `task` (Iterate), or `task-gate` (Gate).

Every generic path in the runner's `TurnCompleted` arm assumes the playhead
belongs to the phase. Three live incidents, three phases, one class:

| Incident | Phase | What happened |
|---|---|---|
| W2-280 gen 5, W2-298 gen 3 | Gate | `record_task_flow_position` rejected `ci-fix` against `task-gate`; the body failed before repair |
| W2-303 gen 4 | Iterate | same rejection against `task` |
| W2-309 gen 2 (PR #1062, 2026-07-17) | Kickoff | no rejection — *worse*: the Kickoff-completion branch ran first, called `enter_iterate()`, replaced the `ci-fix` playhead with a fresh `task` flow, started `task_clarify`, and left wake `cc_be0d0c5bf8ab48289f2b65b2b3cbc6c0` `Claimed` |

#1054 landed the persistence guard for the Iterate/Gate shape
(`if ci_fix_wake.is_none() { record_task_flow_position(..) }`, runner.rs:640).
The Kickoff shape survives it, because Kickoff never validates the cursor — it
*replaces* it. Guarding one write does not make the boundary true; the whole
parent-lifecycle block still runs ahead of the repair's own exit.

## The demo

A Task sleeping in Kickoff on a red PR wakes one ci-fix body. The repair pushes,
the body parks, and `lf task status` reads `waiting` at **kickoff** with the wake
`accepted` — no `task_clarify` turn was ever spent, and the design review the
Task is parked on is exactly where the human left it.

## The interface (read this before rebasing onto this branch)

This PR lands second in a ladder: W2-309 → **W2-294 (this)** → W2-308. W2-308
adds a claimed-wake preempt to `task/runner.rs`, the same file and the same
turn-completion arm. It inherits what is below; it does not resolve around it.

**Where the boundary is.** `exit_bounded_ci_fix_turn(store, session, lease,
harness, wake, CiFixTurnEnd { flow_iteration_completed, status,
head_before_turn }, capture) -> Result<bool>`, in `task/runner.rs`, called from
the `TurnCompleted` arm immediately after the Abandoned check and immediately
before the inner lifecycle `loop`.

**What a later caller may assume.**

- `true` means *the body is over*: the wake is settled, the Session is parked,
  the process is finished. Return `Ok(())` at once; touch nothing else.
- `false` means this body has no wake, or its repair is still mid-flow. It is a
  no-op for every non-repair body (`wake: None`), so nothing needs to ask first.
- It is idempotent-by-position, not by call: it performs terminal writes when it
  returns `true`. Call it once per turn end.
- **Everything below it may assume no ci-fix body is live.** That is the whole
  contract. Kickoff-to-Iterate, the changes-requested gate return, gate
  approval, rotation, and successor steps all sit below and are unreachable for
  a repair body.
- **A new path that reads or writes the Task's lifecycle goes BELOW this call.**
  Above it belongs only to repair-owned decisions that leave the parent cursor
  alone. Placing a lifecycle path above is exactly how this defect class comes
  back — and it comes back *silently*: Kickoff never validated the cursor, so it
  survived #1054's guard with a green suite.
- It decides **when**, never **what**. The verdict is `settle_ci_fix_turn` via
  `decide_open_pr_status`; the incident identity comes from the wake that
  `arm_ci_fix_wake` minted through `current_ci_incident` (ops/task.rs:2685). The
  boundary re-derives neither, and adds no seventh spelling of "is this failing
  head actionable?" — the six that exist are already one too many.

## Approach

**Make the ordering structural, not a rule.** The repair's exit already exists
(`settle_ci_fix_turn`) and its own doc already claims the invariant: *"The body
parks: no gate, no successor step, no PR rotation."* It just sits at the bottom
of the arm, downstream of every path it forbids. Move it up.

One change, two touch points, no new state:

0. **Name it.** The hoist becomes `exit_bounded_ci_fix_turn` — a boundary with a
   spelling, not a rearrangement that only reads correctly in place. W2-308's
   preempt lands in this same arm and must compose with it; an inlined `if` would
   force that Task to re-derive the ordering, and a rebase agent would then pick
   one of the two designs on its own (as it did in W2-287/#1034, silently).

1. **Hoist the ci-fix exit** above the inner lifecycle `loop`. A completed (or
   interrupted) ci-fix flow reconciles its PR, settles its wake, and returns —
   before Kickoff-to-Iterate, before the changes-requested gate return, before
   gate approval, before rotation, before any successor step. Nothing downstream
   can read or write the parent cursor because nothing downstream runs.

2. **Guard the interactive-handoff park** (runner.rs:554). It is the runner's
   third `record_task_flow_position` call site and it is unguarded. A ci-fix body
   *can* reach it: ci-fix boot deliberately skips
   `reconcile_interactive_rendezvous_at_birth` (runner.rs:156-168), so a prior
   body's pending rendezvous survives into the repair, and a completed repair
   turn would park on it and write the `ci-fix` playhead into `task` — the exact
   Iterate bail, by a second route. A repair body neither opened that rendezvous
   nor can resolve it; the next parent body's birth reconcile owns it, and the
   ci-fix exit parks the Task anyway, which is all the park wanted.

The exit condition is `flow_iteration_completed || status == Interrupted` —
byte-for-byte the reachability condition of the tail it replaces, so no ci-fix
turn changes verdict.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is the #1054 guard enough for Kickoff? | No. `enter_iterate()` + `resume_task_phase()` build a *fresh* `task` flow; nothing validates the `ci-fix` playhead it discards. The guard only silences the *validator*, and Kickoff never calls it. | The fix must be ordering, not another guard. |
| Does the Kickoff branch actually run before the exit? | Yes: `if flow_iteration_completed && lifecycle_phase == Kickoff` at runner.rs:658, inside the `loop` that the settle exit (runner.rs:829) sits at the end of. | Hoist above the loop. |
| Does the wake survive the Kickoff bug? | It stays `Claimed`, so it is not lost — but `ensure_child_ci_fix_command` mints no second wake for a spent-or-claimed identity, so the failure is never repaired by anyone. Silent, not loud. | Kickoff needs its own fixture; no error string marks it. |
| Are there other `record_task_flow_position` call sites a ci-fix body reaches? | Three in the runner: 554 (handoff park — **reachable, unguarded**), 641 (guarded by #1054), 1628 (birth reconcile — unreachable, ci-fix boot skips it). | Guard 554. |
| Does hoisting drop a durable instruction absorbed mid-repair? | The pending drain (runner.rs:708) now sits after the exit, so a `FollowUp`/`Steer` claimed during the repair is not applied to the repair body. `absorb_commands` leaves it `Claimed`, and `claim_child_commands_for_lease` reassigns `claimed` rows to the next generation. | Acceptable and already the documented crash contract (`arm_ci_fix_wake`: "a crash mid-turn hands this same command to the next generation"). A bounded repair turn is not the body a Task instruction is for. |
| Would an interrupted ci-fix turn still leave the wake `Claimed`? | Yes — `status == Interrupted` is in the exit condition, and `settle_ci_fix_turn` maps it to `LeaveClaimed`. | Condition must include Interrupted, or an interrupted repair would fall into rotation. |
| Is a Gate ci-fix body at risk beyond the cursor? | Yes, latent: `approved_gate_proposal()?` at runner.rs:761 runs on a Gate ci-fix body whose flow just "completed" and errors when no proposal exists. The hoist removes that path from a repair body entirely. | Bonus, not scope — but do not re-introduce it. |
| Can a freshly-armed wake fall into the exit before its turn runs? | No. The arm site (runner.rs:487-544) `continue 'runner`s while `provider_turn_active`. `ci_fix_wake.is_some()` at the exit means the turn that just ended is this body's repair turn. | No extra guard needed. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Guard each parent transition with `ci_fix_wake.is_none()` | Smallest diff; preserves the mid-repair pending drain | Three guards is a rule a future path must remember. The next lifecycle branch added above the exit re-opens the hole with a green suite — which is exactly how Kickoff survived #1054. |
| A second durable cursor for subflows | Models "two authorities" literally | Explicitly out of scope, and it makes the incoherent state representable rather than impossible. One cursor model. |
| Special-case a Kickoff ci-fix flow | Fixes the reported incident | Fixes one phase and leaves the class. Three phases have now produced the same defect through three different mechanisms. |
| Reconcile the rendezvous at ci-fix birth | Removes the 554 hazard by resolving the handoff | A repair body would then advance the parent's flow past an interactive step — a repair crediting itself as Task progress, the precise thing settlement refuses. |

## Key decisions

- **The exit moves; it does not get copied.** One `settle_ci_fix_turn` call site
  in the runner. The tail's copy is deleted, so there is no ordering to keep in
  sync and no second verdict path.
- **The handoff park is guarded, not reordered.** Its `finish_task_flow_turn` +
  cursor write is parent bookkeeping; a repair has no business in it.
- **Pending commands stay `Claimed` rather than being applied to the repair
  body.** Recovery already owns that shape, and it keeps "one bounded repair
  turn" true.
- **The regression is three real-runner fixtures, not one.** Kickoff, Iterate,
  Gate. Each phase failed by a *different* mechanism, so one phase's proof is
  not evidence about another's — the Gate proof from #1054 passed with the
  Kickoff hole fully present.

## Scope

- In scope: the ci-fix exit ordering in `task/runner.rs`; the handoff-park
  guard; Kickoff and Iterate real-runner regressions beside the existing Gate one.
- Out of scope: failed-head trigger/dedup/attribution (ENG-20, W2-293/W2-299);
  green-CI-before-review (W2-303); proven-empty successor settlement (W2-304);
  revoked lease release (ENG-4).

## Done when

```
cargo test -p loopflow --lib task::runner::ci_fix_lifecycle_tests
```

- `a_kickoff_ci_fix_turn_settles_before_iterate_and_spends_no_lifecycle_turn`
  proves: exactly one provider turn, `lifecycle_phase == Kickoff`, epoch/cursor
  /iteration/gate state untouched, Task `Waiting`, wake `Accepted` exactly once.
- `a_real_ci_fix_turn_preserves_the_iterate_cursor_and_settles_its_wake` proves
  the W2-303 shape completes without `iterate flow is task, but its playhead is
  ci-fix`.
- `a_real_ci_fix_turn_preserves_the_gate_cursor_and_settles_its_wake` (existing)
  proves the W2-280/W2-298 shape.
- `a_ci_fix_turn_settles_past_a_pending_handoff_it_does_not_own` proves the third
  route: the repair settles rather than parking, and the human's rendezvous is
  left pending and unclaimed for the next parent body's birth reconcile.
- Sabotage 1 — delete the `ci_fix_wake.is_none()` guard at the cursor write:
  Iterate and Gate go red on the validator's own message.
- Sabotage 2 — restore the lifecycle transition ahead of settlement (move the
  exit back below the `loop`): Kickoff goes red on a second provider turn and an
  `Iterate` phase.
- No Resume of an unchanged failed wake anywhere; every proof is a fresh
  deterministic lifecycle through `run_task_session_with`.

## Measure

Baseline: 3 phases × 1 live incident each, 0 repairs executed (W2-280 gen 5,
W2-298 gen 3, W2-303 gen 4, W2-309 gen 2). After: a ci-fix wake armed in any
phase spends exactly one provider turn and terminalizes its command. Watch
`ci_incidents.responded_at` non-null with the paired command in a terminal state
— today Kickoff stamps `responded_at` and leaves the command `Claimed`, which is
a response the ledger records and nobody made.
