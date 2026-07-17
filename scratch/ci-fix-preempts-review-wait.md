# A claimed CI wake preempts an interactive review wait

> **Revision 2**, after Project review `ir_15802c67` requested changes. The
> finding was correct and is incorporated: v1 would have fired exclusively on
> `scratch-clear` and repaired it by deleting the design doc under review. The
> mechanism diagnosis survives unchanged; the trigger is now sequenced behind
> `232b91b5`. See "Review log" at the bottom for what moved.

## Problem

A Task parked in an interactive review never repairs its own red PR.

W2-303's #1054 (merged 06:58Z, 2026-07-17) taught the runner that CI must not
interrupt an ordinary active provider turn. The command poll defers `CiFix` while
`provider_turn_active` (`task/runner.rs:361-376`): it claims the command, drops
it from `seen_commands` so a later tick re-sees it, and retains it out of
`absorb_commands`. The arm rides the next `TurnCompleted` boundary instead
(`runner.rs:487`). That is right for ordinary work — a turn that is doing
something ends, and the repair takes the boundary it releases.

An interactive review is not ordinary work. Its turn is the agent *waiting*:
`human_interaction_review_protocol` (`runner.rs:1356`) instructs "Ask bounded
questions and wait for their FIFO follow-up messages." Nothing in that turn is
racing toward a boundary. The repair waits on a turn that is itself waiting on a
human, and the wake is never serviced — the command sits `Claimed` with
`responded_at` null indefinitely.

This serves Developer Efficiency's KR "No Task strands on a dead body": a body
holding a claimed command it can never service is stranded while still breathing.

### Which review waits are actually parked

A *freshly opened* Project-policy review sets `provider_turn_active = false`
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

## What the evidence actually says

The review asked me to compose the fix with its own evidence and to find one real
actionable instance. I censused all 34 rows of `ci_incidents` rather than the four
cited. The answer reframes the work.

**Every wake ever minted, by failing set:**

| Failure set | Incidents | Minted a wake |
|---|---|---|
| `["scratch-clear"]` | 23 | **9** |
| `["rust-test"]` | 7 | 0 |
| `["rust-lint"]` | 2 | 0 |
| `["loopflow-ui-test"]` | 1 | 0 |
| `["rust-test","scratch-clear"]` | 1 | 0 |
| `["rust-test"]` (#1018) | — | **1** |

Ten wakes have ever been minted. **Nine are `scratch-clear`-only.** The reviewer's
four are not a sample — they are nearly the whole population.

`scratch-clear` asserts a land-time precondition: `scratch/` must hold nothing but
`.gitkeep`. It fails on every PR carrying its own design doc — i.e. every Task PR
during kickoff and iterate, which is *exactly* when a review is open. No code
change greens it; `lf pr land` clears `scratch/` as its first act. It reaches the
wake path because branch protection requires only the `tests-result` roll-up,
which `needs: scratch-clear` (`ci.yml:169-173`), so the required read is genuinely
`Failing` and `failing_leaves` is `["scratch-clear"]`.

**Why the actionable ones minted nothing** — this is the part the four-incident
read misses. The minting path went live at ~07:04Z today, right after #1054
merged at 06:58Z. Every incident created after 07:04 minted a wake; all six are
`scratch-clear`-only. All ten unminted actionable incidents are from 05:23–05:35Z
and *predate the mechanism*. The one actionable wake ever minted (#1018,
`rust-test`) was responded and superseded — serviced correctly, no review parked.

**So: zero measured instances of an actionable failure during a parked review.**
I am saying that plainly, as asked. Two honest readings follow, and they point
opposite ways:

- The minting path is ~2 hours old, so "zero" is weak evidence of rarity.
  Actionable failures plainly occur — eleven incidents — they simply have not
  coincided with a parked review since minting began. The hole is structural and
  will present.
- The fix is therefore **prophylactic**, not a repair of measured harm. That kills
  any urgency argument for landing it ahead of its dependency.

### The composition failure in v1

Today a parked review leaves an unserviceable wake `Claimed` — inert and wasteful.
Under v1, that same wake would preempt: interrupt the review, run a bounded repair
turn against a check no action can green, where the only move that "succeeds" is
deleting the `scratch/` design doc the reviewer is actively reading. Given the
census, that is not an edge case — it is 9 of 10 wakes, and 6 of 6 since minting
went live. v1 converts an inert stranded command into an active turn whose sole
path to success is destroying the artifact under review.

This is the ENG-4 shape from wave memory: a fix that rebuilds the bug inside its
own remedy, invisible because each sentence is individually true. I verified every
claim in v1 in isolation and never asked what the preemption and the wake's
*content* do together.

## Approach

**Preempt only a wake that is current, and let `232b91b5` decide what "current"
means.** The choice is the review's option (a) — sequence behind the
classification, keep scope — and it is clean for a routing reason rather than a
preference.

Both the mint and my preempt read the same authority:

```
ci_fix_wake_kind        (mint)     -> current_ci_incident   [ops/task.rs:2724]
current_ci_incident_identity (new) -> current_ci_incident   [my helper]

current_ci_incident(pr) = pr.fresh_ci().filter(|r| r.wake_legal()).and_then(ci_incident)
                                                     ^ ops/task.rs:2685-2688
```

`232b91b5` makes a land-time-precondition check non-actionable somewhere in that
chain — `MergeGateReading` (`ops/pr.rs:550-589`), `wake_legal`
(`task/mod.rs:326`), or `ci_incident` (`ops/task.rs:2781`). Wherever it lands,
`current_ci_incident` returns `None` for a `scratch-clear`-only head. My currency
check calls that exact function, so:

> **A `scratch-clear`-only wake yields no current incident → `holds_current_ci_fix_wake`
> is false → no interrupt, no turn, `scratch/` untouched.** Inherited, with zero
> coordination.

No name list, no parallel classifier, no exclusion logic of my own. This is the
wave-memory rule "a shape beats a rule": the bad preemption becomes unrepresentable
upstream rather than defended downstream. It is also why I decline option (b) —
carrying the exclusion myself would build a second classifier beside the one
`232b91b5` owns, and two derivations of "actionable" that drift is precisely the
`arm_ci_fix_wake`/`ci_fix_wake_kind` identity-drift hazard already documented at
`ops/task.rs:2719-2723`.

### Why not (c), settle rather than service

Worth the paragraph the review asked for, because it is nearly right. (c) is
correct for a wake that must never be serviced — and wrong as *my* fix, in both
directions. For a **non-actionable** wake it is redundant: `232b91b5` prevents the
command from ever being minted, which is strictly better than minting one and then
settling it. For an **actionable** wake it is harmful: settling without a turn
spends the incident identity for good — `ensure_child_ci_fix_command` mints no
second wake for a spent identity ("Any terminal state spends the incident identity
for good", `runner.rs:2505-2508`) — so a real `rust-test` failure during a review
would be permanently unrepaired. That is fail-open, and it recreates W2-290's shape
in a new place. The residue (c) genuinely targets — the already-stranded
`cc_fe73f4ed` / `cc_35e8409e`, both `scratch-clear`, both `Claimed` right now —
is real, but it is `232b91b5`'s to settle: they exist because they were minted, and
that is the defect being fixed. I am not widening to take them.

### The dependency is hard

**This PR must not merge before `232b91b5`.** Landing first makes every preemption
a `scratch-clear` preemption — the exact harm above. Stated as a merge-order
constraint, not a hope: the negative regression below *fails* until `232b91b5`
lands, so the suite enforces the ordering rather than trusting me to remember it.
That is deliberate — a red test is a better gate than a note.

### The change

Three pieces, ~25 lines. Unchanged from v1 in mechanism; the currency check now
carries the classification's weight.

1. **`current_ci_incident_identity(store, session) -> Result<Option<String>>`** —
   extracted verbatim from `arm_ci_fix_wake`'s existing currency read
   (`runner.rs:2392-2397`). One authority for "what is this PR failing *now*",
   read by both the arm and the preempt.

2. **`holds_current_ci_fix_wake(store, session, &[ChildCommand]) -> Result<bool>`** —
   the read-only half of the arm's selection. Answers "would preempting actually
   reach a repair" **without** superseding stale wakes or stamping
   `mark_ci_incident_responded`. Those are writes; they belong at the arm, and
   running them mid-turn is what the deferral exists to prevent.

3. **`review_preempted: bool`** — set at the interrupt, cleared where
   `provider_turn_active = false` at `TurnCompleted` (`runner.rs:469`). One
   interrupt per provider turn.

```rust
let (wake, commands) = if provider_turn_active {
    let claimed = claim_commands(&store, &session, lease, &mut seen_commands).await?;
    let (ci_fix, rest): (Vec<_>, Vec<_>) = claimed
        .into_iter()
        .partition(|command| matches!(&command.kind, ChildCommandKind::CiFix { .. }));
    for command in &ci_fix {
        seen_commands.remove(&command.id);
    }
    // An interactive review turn is not ordinary work: it is the agent waiting,
    // so no boundary is coming to release the repair. Take it once — but only
    // for a wake the PR's *current* reading still names. That check is what
    // keeps a non-actionable failure (a land-time precondition like
    // `scratch-clear`, which no repair can green) from spending a review turn:
    // it yields no current incident, so it never reaches here.
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

Everything downstream already exists and is tested: the arm at `runner.rs:487`
claims through `claim_and_arm_ci_fix`, starts the bounded flow in the same
generation, and clears the *local* `interaction_review` while the durable record
stays `Active`.

## The demo

A Task sits in an open review while `rust-test` fails on its PR. Today the `CiFix`
command reads `Claimed` and the incident `responded_at` null, indefinitely. After
this change the review turn is interrupted once, the bounded ci-fix flow starts in
that same generation on that same command, the incident stamps `responded_at`, and
the review record is still `Active` for the next generation to resume. The same
Task with a `scratch-clear`-only failure is untouched: no interrupt, no turn, the
design doc intact.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are the cited incidents actionable? | **No — 9 of 10 wakes ever minted are `scratch-clear`-only.** Land-time precondition; no code change greens it. | v1 was harmful. Preempt gated on the shared currency authority; sequenced behind `232b91b5`. |
| Is there one real actionable-during-review instance? | **No.** Minting went live ~07:04Z today; all 6 wakes since are `scratch-clear`. The 10 actionable incidents predate it; the 1 actionable wake (#1018) was serviced. | Fix is prophylactic. Kills any urgency to land ahead of the dependency. |
| Can I inherit the classification instead of duplicating it? | Yes. Mint and preempt both route `current_ci_incident` (`ops/task.rs:2685`). | No name list, no second classifier. Option (b) declined. |
| Does `scratch-clear` reach the wake path today, and where? | `merge_gate_state` → required `tests-result` (`needs: scratch-clear`) → `state = Failing` → `wake_legal` true → incident with `failing_leaves = ["scratch-clear"]`. | Confirms `232b91b5`'s fix lands in the chain my check reads. |
| Is the review turn actually an active provider turn? | Yes for the three shapes tabled above; a fresh Project review is not (`runner.rs:1452-1469`) and already arms. | Guard is `provider_turn_active && interaction_review.is_some()`. |
| Can I reuse `arm_ci_fix_wake` to test currency? | No — it supersedes stale wakes and stamps `mark_ci_incident_responded` (`runner.rs:2415-2468`). | Split the pure read out; every write stays at the arm. |
| Does an interrupted review turn reach the arm? | Yes. `TurnCompleted` clears `provider_turn_active`; the arm at `runner.rs:487` runs before `resume_interrupted_flow`, which is false under a review (`flow_turn_active` is false, `runner.rs:703`). | No new arming path. |
| What if the wake goes stale between the check and `TurnCompleted`? | Benign. Nothing arms; control reaches `runner.rs:729` — `if interaction_review.is_some() { continue 'runner }`. The body idles with the review open; a follow-up restarts the turn. | No compensating logic. The fallback is a state the runner already reaches. |
| Does the durable review survive the repair? | Yes. `interaction_review = None` at `runner.rs:508` is a local. The ci-fix body exits via `settle_ci_fix_turn` (`runner.rs:829`); the next generation reopens the `Active` record (`runner.rs:228-256`). | Assert it in the test. |
| Does this break W2-303's rule? | No. `a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle` has no review, so the guard is false and it stays at `interrupts == 0`. | That test is the regression. |
| Should a failed interrupt fail the body? | Yes, propagate. Matches the control path (`runner.rs:3876-3889`). Swallowing it sets no bit and re-interrupts every 200ms against a harness that cannot be interrupted. | `harness.interrupt().await?`. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| **(b) Carry the actionable-leaf exclusion myself** | Lands independently | Builds a second "actionable" derivation beside `232b91b5`'s. Two derivations that drift is the documented identity-drift hazard (`ops/task.rs:2719-2723`). The currency check already inherits the right answer. |
| **(c) Settle the wake without a turn** | Smaller; fixes the stranded command directly | Redundant for non-actionable wakes (`232b91b5` prevents minting) and fail-open for actionable ones: settling spends the identity for good, so a real failure during review is never repaired. |
| Ship v1 now, fix the trigger later | Unblocks this Task | 9 of 10 wakes are `scratch-clear`; the interim behavior deletes design docs under review. Strictly negative. |
| Arm ci-fix during the review turn, no interrupt | Skips a boundary | The turn is live; the seed races the review transcript. The parallel repair path W2-303 removed. |
| Deliver the wake as provider text | No interrupt, no new state | Turns a durable command into prose. Nothing settles it, `responded_at` stays null. Forbidden. |
| Accept the command early, repair in a successor | No interrupt | Accepting before a repair is a lie the ledger cannot walk back, and it spends the identity. Recreates W2-290. |
| Complete/close the review, repair, reopen | Clean state machine | A `Completed` review is write-once and a wrong disposition is terminal. Never dispose a review to unblock plumbing. |
| Time-bound the review wait | No CI coupling | A timer that guesses. A review with no red PR should wait as long as it likes. |

## Key decisions

**The trigger is inherited, not written.** The one line carrying the review's
finding is `holds_current_ci_fix_wake`, because it routes through
`current_ci_incident`. That is the whole defense against the `scratch-clear`
composition, and it is defense by construction rather than by rule.

**The guard is `interaction_review.is_some()`, not "the reviewer is human."** A
resumed Project review parks identically (`runner.rs:249-256`). Keying on reviewer
kind would split the two shapes.

**Currency is checked before interrupting, not after.** Interrupting first and
letting the arm decide would spend a real review turn on a wake the arm then
supersedes. Cost is one `active_task_pr` read per tick while a review is parked
with a claimed wake — a rare, already-degenerate state.

**One bit over a new type.** `review_preempted: bool` beside
`turn_had_durable_side_effect`, cleared at the line that clears
`provider_turn_active`. Per-turn scope falls out of where it is cleared, and the
degenerate loop cannot run: after `TurnCompleted` either the wake armed
(`interaction_review` is `None`) or it went stale (`holds_current_ci_fix_wake` is
false). Both close the guard.

**The pure/impure split is load-bearing.** "Is there a current wake" and "arm the
current wake" were one function, so the poll could only ask by committing to the
answer. Extracting the read makes the preempt decidable mid-turn without touching
the ledger — and is what lets the classification be inherited.

## Scope

- In scope: the preempt branch in the command poll; the two extracted read
  helpers; the `review_preempted` bit; the tests below.
- Out of scope: **`scratch-clear` itself and all `scratch/` handling — the check is
  correct**; the actionability classification (`232b91b5`); settling the two
  already-stranded `scratch-clear` commands (`232b91b5`); the fresh-Project-review
  idle path (already arms); ci-fix flow content, seeding, bounds, or settlement;
  review policy, disposition, or the human-review gate; W2-303's ordinary-work
  rule, which this preserves.
- Depends on: **`232b91b5` must merge first.**

## Done when

`cargo test -p loopflow --lib task::runner`, plus `cargo fmt` and
`cargo clippy -p loopflow --lib --tests -- -D warnings` (the `--tests` target
matters: a lib-only clippy does not lint in-file test modules — W2-300 lost a CI
run to exactly that).

New in `task/runner/ci_fix_lifecycle_tests.rs`, pinning **both** directions:

- **`a_parked_review_wait_is_preempted_once_by_an_actionable_wake`** — a harness
  whose first `send_input` opens a review turn, queues a ci-fix command for a
  `rust-test` failure, and **never** sends `TurnCompleted`; its `interrupt()`
  counts, then emits `TurnCompleted { Interrupted }` after ~500ms so several 200ms
  poll ticks pass while the turn is still active and already preempted. Asserts
  `interrupts == 1` (once, across ticks), `sends == 2` (the ci-fix seed follows),
  the settled command is the same id claimed by the same generation,
  `responded_at` is stamped, and the durable review row is still `Active`.
- **`a_scratch_clear_only_wake_never_preempts_a_review_wait`** — the same parked
  review, with the head failing `scratch-clear` only. Asserts `interrupts == 0`,
  `sends == 1` (no repair turn), and `scratch/` is untouched. **This test fails
  until `232b91b5` lands** — that is the merge-order gate, enforced by the suite.
- **`a_stale_wake_never_preempts_a_review_wait`** — parked review, PR moved to a
  head whose current identity differs from the claimed wake. Asserts
  `interrupts == 0`. Passes today; guards the currency check independently of the
  classification.

Existing, must stay green:

- **`a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle`** — no review
  open, `interrupts == 0`: ordinary active work is never interrupted.
- **`an_interrupted_turn_leaves_the_wake_claimed`**,
  **`a_crash_after_arm_reclaims_the_same_command_and_reselects_ci_fix`** — recovery
  unchanged.

Sabotage proofs (run each, confirm red — a test that passes with the fix removed
pins the fixture, per MEMORY's "sabotage the code the test names"):

| Remove | Must turn red |
|---|---|
| the whole preempt block | `a_parked_review_wait_is_preempted_once_by_an_actionable_wake` (hangs to timeout) |
| `interaction_review.is_some()` | `a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle` |
| `!review_preempted` | `a_parked_review_wait_...` on `interrupts == 1` |
| `holds_current_ci_fix_wake(..)` | `a_scratch_clear_only_wake_never_preempts_a_review_wait` **and** `a_stale_wake_never_preempts_a_review_wait` |

The last row is the point the review made: with only an actionable-wake test, the
destructive interaction is fully present and nothing turns red. Both negative
tests exist so the currency check cannot be deleted quietly — one guards it
against the classification, the other against staleness.

## Measure

Baseline, on the live store: `CiFix` commands sitting `Claimed` against a live
generation whose incident has `responded_at` null — `cc_fe73f4ed` and
`cc_35e8409e` today, both `scratch-clear`, both of which `232b91b5` prevents from
ever being minted.

The measure for *this* change is narrower and honest about the sample: **once an
actionable failure lands on a PR with a parked review, its wake reaches a terminal
state within one review turn's interrupt rather than never.** No such incident
exists yet (see the census), so this is a forward measure, not a before/after. The
count-based framing is deliberately avoided — per MEMORY's ENG-4 correction,
nothing sweeps the table, so a snapshot always catches transients. The claim is
that **no claimed wake is permanent**, not that any count reaches zero.

Companion signal owned by `232b91b5`, worth watching together: wakes minted for
`scratch-clear` should go to zero (from 9 of 10).

## Review log

| Finding (`ir_15802c67`) | Resolution |
|---|---|
| Every cited incident is `scratch-clear`-only; servicing it sooner is the wrong direction | Confirmed and widened: censused all 34 incidents — 9 of 10 wakes ever minted are `scratch-clear`-only. v1's composition failure named explicitly in "The composition failure in v1". |
| Choose a shape: (a) sequence, (b) carry the exclusion, (c) settle | **(a)**, for a routing reason: mint and preempt share `current_ci_incident`, so the classification is inherited. (b) declined — second classifier, drift hazard. (c) declined with the paragraph asked for — fail-open for actionable wakes, redundant for the rest. |
| Do not name `scratch-clear` as a literal | No name list anywhere. The only mention is in a comment explaining *why* the currency check carries the weight. |
| Regression must pin both directions and fail on today's code | Three tests; `a_scratch_clear_only_wake_never_preempts_a_review_wait` fails until `232b91b5` lands, which makes the suite the merge-order gate. Sabotage table's last row is the trap the review identified. |
| Find one real actionable-during-review instance, or say plainly there is none | **There is none.** Stated in "What the evidence actually says", with the ~07:04Z cutover that explains why, and both honest readings of what "zero" means. |
| Do not widen scope to `scratch-clear` or `scratch/` handling | Both listed under out-of-scope, including the two already-stranded commands. |
