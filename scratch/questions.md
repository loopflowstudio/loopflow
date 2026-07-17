# Open questions — W2-308

## Blocking — needs a Project sequencing decision

**This PR must not merge before W2-309** (`232b91b5`, "A ci-fix wake arms on
scratch-clear"). Landing first makes every preemption a `scratch-clear`
preemption: 9 of 10 wakes ever minted are `scratch-clear`-only.

**W2-309 is open, unassigned, with no branch and no PR.** So this Task can build
and publish but cannot land, and nothing inside my PR can enforce that — see
below. This is the one decision I cannot take myself:

- I cannot implement W2-309 here. It is a separate Linear Task, and `task_clarify`
  forbids selecting backlog work or starting another Session. Its Done-When names
  a classifier change in `ops/pr.rs` plus its own regression — a real slice, not a
  line I can borrow.
- So the merge order is the Project's to sequence. My PR states the dependency and
  I will not land it. If W2-309 is dispatched and lands, this needs no change: the
  currency check inherits its classification through `current_ci_incident`.

**R2's enforcement idea was removed because it was harmful, not merely clumsy.** A
regression that "fails until W2-309 lands" fails via `rust-test` — an *actionable*
failure, which W2-309 deliberately keeps arming. It would mint a real wake and
burn a body whose only route to green is deleting my test. I would have shipped
the exact defect class this Task exists to prevent, inside the section arguing
against it. No red test.

**If W2-309 is rejected**, or lands at the enqueue site (`ci_fix_wake_kind`) rather
than in the `current_ci_incident` chain, the inheritance argument fails and this
design needs revisiting — most likely by adopting the review's option (b) and
reusing W2-309's predicate at the preempt.

## Assumptions, proceeding

1. **`232b91b5` fixes actionability inside the `current_ci_incident` chain** —
   `MergeGateReading` (`ops/pr.rs:550`), `wake_legal` (`task/mod.rs:326`), or
   `ci_incident` (`ops/task.rs:2781`). All three are read through
   `current_ci_incident` (`ops/task.rs:2685`), which my currency check calls, so
   any of them gives the inheritance. If it instead filters at the *enqueue* site
   only (`ci_fix_wake_kind`), the mint stops but a previously-minted wake could
   still read current — in that case I add the leaf check at the preempt after
   all, reusing `232b91b5`'s predicate rather than writing a second one.

2. **A failed `harness.interrupt()` fails the body.** Chose `?` over `let _ = ...`
   to match the control path (`runner.rs:3876-3889`); swallowing it leaves
   `review_preempted` unset and re-interrupts every 200ms against a harness that
   cannot be interrupted. Cost: a broken interrupt kills a body holding an open
   review. The review record is durable and `Active`, so a successor generation
   resumes it — the same recovery every other body failure uses.

3. **One interrupt per provider turn, not per body.** Clearing `review_preempted`
   at `TurnCompleted` lets a review that resumes and later meets a *new* failing
   head be preempted again. Cannot loop: after `TurnCompleted` either the wake
   armed or it is stale, and both close the guard.

4. **The stale-wake fallback is the pre-existing idle state.** If a wake goes stale
   between the currency read and `TurnCompleted`, nothing arms and the body idles
   at `runner.rs:729` with the review open. Not adding compensating logic for a
   state the runner already reaches today.

5. **Not touching the fresh-Project-review path.** It sets
   `provider_turn_active = false` and already arms through the idle poll. The
   directive's framing implies all reviews park; measured, they do not.

## Noted, not mine

- **The two stranded `scratch-clear` commands** (`cc_fe73f4ed`, `cc_35e8409e`, both
  `#1059`, both `Claimed` with `responded_at` null) belong to `232b91b5` — they
  exist because they were minted, which is the defect it fixes. Settling them from
  here would be option (c), which is fail-open for actionable wakes.
- **A stale wake claimed by a parked review body is not superseded until the review
  completes** — `arm_ci_fix_wake` supersedes stale wakes only when it runs, which
  needs an idle poll a parked review never reaches. Benign (a stale wake should not
  be repaired) and self-resolving at review completion, so out of scope. Recording
  it because it is the same "stranded `Claimed` command" family and a future reader
  may expect this Task to have covered it.
