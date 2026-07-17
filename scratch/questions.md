# Open questions — W2-308

## R6 — yielded again; the sibling is progressing and is doing the right thing

Second consecutive wave-seed turn to yield. My predecessor (3916) ended, the
runner spawned me (52609) under the *same* generation-3 `lf __task`, and sibling
**3816 is still alive at 4:28** holding the review prompt. So the yield is not a
one-off: the runner starts a new flow-step turn every time the previous one ends,
while the long review turn keeps running, and each new turn must stand down.
**That is one body burned per spawn** until 3816 finishes — the real cost of the
concurrent-turn defect, and the reason it is worth fixing rather than tolerating.

Measured this turn, read-only (no code touched, no commit):

- The sibling **already fixed the blocker**: duplicates gone (2731 → 2544 lines,
  one definition each), `runner.rs` comments collapsed to the seam (44 lines
  touched, net −12), and `holds_current_ci_fix_wake`'s rustdoc now states the
  invariant in two lines instead of arguing. That is review items 1 and 2.
- It is **not stuck**: it last wrote at 10:35:59Z and immediately started
  `cargo check -p loopflow --lib --tests` (pid 19434) — notably *with* `--tests`,
  the invocation whose absence let the duplicates through. Three `rustc` are live,
  so it is compiling, not hung.
- The build lock is contended but not by an ownerless corpse: pid 42954
  (`loopflow.a-ci-fix-wake-arms`, 1:07:24) still has a **live zsh parent**, so my
  earlier "orphan in a completed Task's worktree" read was wrong about *that* pid
  and I did not kill it. A third cargo (76155) is a `--release` build of `lf` in
  the main repo — the fleet-staleness work, unrelated to this branch.

**Nothing here changes the plan.** The sibling owns items 1–4; both dependencies
are on main (W2-309 `15e441e69`, W2-294 `062bd1e4c`); the hold's condition is met.
One fact the sibling cannot know from its own prompt: its review text says #1063 is
"queued at position 1, AWAITING_CHECKS", but **#1063 merged at 10:37:23Z**. That
does not misdirect it — the review told it to confirm the merge before rebasing,
and a rebase onto main now picks up `062bd1e4c` regardless.

## R5 — this turn yielded: a sibling provider turn owns the worktree

**The review's blocker is real and correctly diagnosed.** At `HEAD` (`be94394e2`)
`ci_fix_lifecycle_tests.rs` is 2731 lines and defines
`a_parked_review_wait_is_preempted_once_by_an_actionable_wake` **twice** (2317 and
2557) and `a_scratch_clear_only_wake_never_preempts_a_review_wait` twice (2454 and
2703). Cause: a `cat >> … <<'RUST'` heredoc that ran twice. Why nothing caught it —
and this sharpens a rule already in MEMORY for clippy, now confirmed for `check`:
**`cargo check -p loopflow --lib` does not compile `#[cfg(test)]` modules**, so its
green result said nothing about the tests, and every actual `cargo test` run died
on the shared cross-worktree build lock. The branch has never been compiled with
its tests. The review is right that the publication hold hid this.

**I did not fix it this turn, deliberately.** Mid-turn the worktree changed under
me: `git status` was clean at turn start, then both `runner.rs` and
`ci_fix_lifecycle_tests.rs` went ` M` with the duplicates already deleted (−187)
and `runner.rs` reduced. No rebase was in flight (`git reflog` clean, no
`.git/rebase-merge`), so this was not the known mid-rebase false alarm — it was
**another writer**:

```
91402  lf __task ts_26312c2689d741c0b1e27a1afd57d009 --generation 3
 ├─ 3816  claude -p <interaction_review_completed …>   cwd = this worktree, alive
 └─ 3916  claude -p <lf:wave …> + task_clarify seed    cwd = this worktree  ← me
```

Both are children of **one generation**, spawned one second apart, both writing
this worktree. My sibling holds the review prompt and is already performing the
reduction the review asked for. Editing or committing now would corrupt or capture
its in-flight state under my message, so this turn touched no code and made no
commit — only this note and a wave-memory entry. That is the "concurrent editing
corrupts a file" hazard from MEMORY, occurring *inside* a single generation, where
no dispatch discipline can prevent it.

**The reduction work is therefore owned by turn 3816, not by this note.** What it
must still satisfy, from the review, is recorded here only so it survives if that
turn dies:

1. delete the duplicate definitions (E0428 ×2) — in progress, −187 already;
2. cut `runner.rs` to the minimal seam (partition, currency check, one interrupt,
   reset on `TurnCompleted`) and collapse the authority comments;
   `holds_current_ci_fix_wake`'s rustdoc must state the invariant, not argue;
3. two behavior proofs on **one** compact harness — reuse the existing harness
   rather than minting `ReviewWaitHarness` beside it;
4. sequence: wait for **#1063 to MERGE** (read `mergeQueueEntry`, not
   `autoMergeRequest` — null there means queued, per MEMORY; confirm `merged=true`
   with a `mergeCommit`), then rebase, preserving W2-294's settlement ordering —
   grep for the **hoisted `if let Some(wake) = ci_fix_wake.as_ref()` branch
   position** above the parent lifecycle loop, *not* for `settle_ci_fix_turn`,
   which already exists on main and is therefore not evidence W2-294 landed.
   Resolve any conflict by hand; no rebase agent. Then publish and land.

### The hold's release condition is now MET (verified, not inferred)

**W2-294 / PR #1063 is MERGED**: `state=MERGED`, `mergedAt=2026-07-17T10:37:23Z`,
`mergeCommit=062bd1e4cfd78f0354dcdfe1d5955adcc6e86784`. Read from `state` +
`mergeCommit` directly, which is what the review asked for and what MEMORY
requires — `autoMergeRequest` reading null would have meant *queued*, not
disarmed. So directive v3's bar ("do not publish, land, or arm merge until W2-294
completes … and its boundary is on the integration base") is satisfied, and the
next step is the rebase onto `062bd1e4c` — owned by the sibling turn, not this
note. Both dependencies are now on main: W2-309 `15e441e69`, W2-294 `062bd1e4c`.

## Hold v3 (incorporated) — waiting on W2-294

Directive v3 replaces v2: no publish, land, or arm until **W2-294** completes its
Project review and its named reusable ci-fix settlement boundary is on the
integration base; then rebase or adapt onto it. Incorporated, verified by re-read
(`current=3, incorporated=3` — v2 stays uningested, correctly, since v3 replaces
it). No PR-state mutation has occurred: this branch has never been published.

**W2-309 is done and this design's central claim is now confirmed on main, not
inferred.** #1062 merged as `15e441e69` carrying the implementation (it shed its
`scratch/` files at land, which is why an earlier read saw only a design). This
branch is rebased onto it; `git merge-base --is-ancestor 15e441e69 HEAD` passes.
The landed `wake_legal` (`task/mod.rs:361`) returns false when every failing check
is a `land_time_precondition`, exactly inside `current_ci_incident` — so
`holds_current_ci_fix_wake` inherits the exclusion with no name list here.

### The W2-294 collision is real, and it cuts both ways

W2-294 ("Keep ci-fix subflow playheads out of the durable Task cursor") owns
`settle_ci_fix_turn` as its boundary **and** owns both fixtures this Task touches:
its Done-When requires "a deterministic Iterate fixture" (the W2-303 shape) and "a
deterministic Gate fixture" (the W2-280/W2-298 shape). My R4 change moves
`a_live_generation_holds_ci_fix_until_its_provider_turn_is_idle` from Gate to
Iterate and adds a Gate review fixture — same file, same coordinates. Adapting
onto W2-294 rather than racing it is right, and is what the hold instructs.

**A finding W2-294 needs, which I cannot deliver directly (tier boundary — routing
it through this doc, which the Project reads).** Checked against **PR #1063** as
published, not against the Task description. It touches `runner.rs` and
`ci_fix_lifecycle_tests.rs` — both files this branch edits — so a textual conflict
is expected and fine. The substantive part is which coordinates its fixtures chose.

**Interactivity of every lifecycle step, measured from the flow YAML and the skill
frontmatter** (the table this Task had to build anyway):

| Flow | 0 | 1 | 2 |
|---|---|---|---|
| `task` (Iterate) | `task_clarify` — ordinary | `task_pursue` — ordinary | `task_mutate` — ordinary |
| `task-kickoff` | `kickoff` — ordinary | **`review-design` — INTERACTIVE** | — |
| `task-gate` | **`demo` — INTERACTIVE** | **`code-review` — INTERACTIVE** | `gate` — ordinary |

Under `TaskLifecyclePlan::standard` (what `make_task` uses) Kickoff and Gate are
both `Require`, so any interactive step there opens a **Human** review and leaves
`provider_turn_active = true`.

Applying that to #1063's two new fixtures:

- `a_real_ci_fix_turn_preserves_the_iterate_cursor_and_settles_its_wake` — Iterate
  `phase_cursor = 1` → `task_pursue` → **ordinary. No interaction with this
  Task's preempt.** Correct as written.
- `a_kickoff_ci_fix_turn_settles_before_iterate_and_spends_no_lifecycle_turn` —
  Kickoff `phase_cursor = 1` → **`review-design`, interactive** → Human review with
  an active turn. **That fixture is parked in a review**, which is precisely the
  state this Task's preempt fires on.

What that means, stated at the confidence it deserves: #1063's new tests do not
appear to assert `interrupts == 0` (unlike W2-303's, which does), so the Kickoff
fixture may well stay green — the preempt would interrupt its review turn, the arm
would run, and the repair would still settle, which is roughly what it asserts
anyway. But its harness may also drive its own `TurnCompleted`, and a preempt plus
a self-driven completion on one turn is an interaction neither Task has run. I am
not claiming it breaks; I am claiming it is **unverified and cheap to verify** —
which is exactly what the hold's "rebase or adapt onto that boundary" buys.

**Adaptation plan once #1063 lands** (no action needed from W2-294): rebase, run
its two fixtures against this preempt, and if the Kickoff one is disturbed, move it
to `phase_cursor = 0` (`kickoff`, ordinary) — which preserves its intent exactly,
since its subject is the *cursor*, not the review. That mirrors this Task's own R4
correction of W2-303's fixture and costs W2-294 nothing.

## Superseded — the W2-309 sequencing decision (resolved)

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
