# Open questions — W2-308

## Blocking — needs a Project sequencing decision

**This PR must not merge before W2-309** (`232b91b5`, "A ci-fix wake arms on
scratch-clear"). Landing first makes every preemption a `scratch-clear`
preemption: 9 of 10 wakes ever minted are `scratch-clear`-only.

**W2-309 is now live: PR #1062, design published, Project review open.** It is a
kickoff/design PR, so the classifier is not implemented yet. This Task can build
and publish but must not land until #1062's implementation merges. That order is
the Project's to sequence; my PR states it and I will not land.

- I cannot implement W2-309 here. It is a separate Linear Task with its own live
  Session and PR, and `task_clarify` forbids selecting backlog work. Racing it
  would duplicate an in-flight design (the exact #1030 trap in wave memory).

### The inheritance claim is now VERIFIED, not assumed

Read from #1062's design rather than inferred, and it lands exactly where this
design needs it:

- **W2-309 puts the fix in `wake_legal()`** (`task/mod.rs:326`), calling it "the
  single legality question". `current_ci_incident` is
  `fresh_ci().filter(|r| r.wake_legal()).and_then(ci_incident)`
  (`ops/task.rs:2685-2688`) — so a `scratch-clear`-only head yields **no current
  incident**, `holds_current_ci_fix_wake` returns false, and this PR's preempt
  never fires. Inherited with zero coordination, as designed.
- **It explicitly rejects the enqueue site** (`queue_ci_fix_command`) — "answering
  it in one caller leaves `ci_fix_restart_bar` believing a scratch-clear head
  warrants an automated restart". That was assumption 1's risk branch below, and
  it is now closed: the fix cannot land somewhere my check does not read.
- **It also rejects filtering `MergeGateReading::failing_leaves`** (it would delete
  the failure from the *observation*, reporting a red head with no named checks).
  So `failing_leaves` keeps naming `scratch-clear` and the suppression happens at
  legality — which is the layer my check consults.
- Corroboration that we are reading the same seam: #1062's own de-risking cites
  "`runner.rs:2396` re-derives through `current_ci_incident`, which respects
  `wake_legal`" — that is the exact line `current_ci_incident_identity` extracts.

**Consequence for the two stranded commands** (`cc_fe73f4ed`, `cc_35e8409e`),
cleaner than R3 stated: once W2-309 lands, `current_ci_incident` is `None` for
their head, so `arm_ci_fix_wake` supersedes them as stale at its next run and my
preempt ignores them. Neither Task needs a sweeper.

**One inherited behavior worth naming:** W2-309 keeps `failing_checks.is_empty()`
legal — "fail toward waking; a filter that suppresses unknown failures is a mute
button". So an unnamed failing head still wakes, and this preempt will fire for it
during a review. That is correct and deliberate on both sides: an unnameable
failure is not a proven non-actionable one.

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

1. ~~**`232b91b5` fixes actionability inside the `current_ci_incident` chain.**~~
   **RESOLVED** by reading #1062: it lands in `wake_legal`, inside the chain, and
   explicitly rejects the enqueue-site variant that would have broken this. See
   the verification above. No fallback to option (b) is needed.

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
