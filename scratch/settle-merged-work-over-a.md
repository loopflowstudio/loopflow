# Settle merged work over a proven-empty successor

## Problem

A Task whose real work merged cannot close, because the lifecycle minted an
empty successor PR behind it and the completion gate refuses to settle over an
unpublished PR.

W2-280 is the live shape. PRs #1036 and #1048 merged and carry the work. The
lifecycle then rotated to sequence 4 on
`jack-heart/retire-the-parallel-spend-recorders-4`, where `head == base ==
3be919fc0`: zero commits, clean tree, never published. `lf task complete` refuses
with

```
pull request sequence 4 is unpublished; publish and merge it or run `lf pr abandon`
```

Neither branch of that sentence is available. Publishing would open a PR with no
commits — GitHub auto-closes a zero-commit PR, and asking for review on it
misstates the Task. Abandoning is worse: `abandon_task_pr` (ops/task.rs:1761)
sets the Session to `Waiting` with *"another PR may follow"*, and the next
`ensure_working_pr_with_authority` rotates a fresh empty PR N+1. Abandoning PR N
mints PR N+1. That is the loop's feed, not a fix — measured on ENG-7, where PR 2
and PR 3 were both empty, and on W2-280, where generation 8 stalled 1804s against
the successor's unanswerable demo gate, went `revoked -> finished(superseded)`,
and generation 9 relaunched onto the same gate. It repeats indefinitely, one full
Opus body per cycle.

This is the settle-side counterpart to #1050 (W2-300), which shipped the read-side
authority: `committed_follow_up_range` returns an explicit tri-state where only
`ProvenEmpty` permits completion, and `Range`/`Unprovable` block. That answers
"is there work past the merged tip." Nothing yet answers "may this empty
successor be discarded so the merged predecessor can settle."

Beneficiary: every Task that merges its work through a serial rotation — i.e.
Developer Efficiency's *"No Task strands on a dead body"* KR, which today counts
W2-280 against it while the code it was measuring is already on main.

## The demo

On W2-280's exact shape — #1048 merged, sequence 4 unpublished with `head ==
base`, clean tree:

```
$ lf task complete W2-280 --summary "spend recorders retired; #1036 and #1048 merged"
W2-280 completed
```

Sequence 4 is terminal and non-authoritative, no body generation is reserved, and
no PR 5 appears. Run `lf task status W2-280` afterwards and it still reads
`completed` — the reconcile does not walk it back.

Today the same command prints the refusal above, and the Task keeps burning.

## Approach

One function, called from the two paths that mutate a Session into `Completed`.

### 1. Generalize the cut, not the tri-state

`committed_follow_up_range` already answers "what is on this branch past a cut?"
The only thing that differs for an unpublished successor is *which commit is the
cut*:

| PR shape | Cut | Meaning of commits past it |
|---|---|---|
| Merged, published | `github.head_sha` — the tip GitHub merged | follow-up owned by no PR (#1050) |
| Working, unpublished | `base_commit` — the fork point recorded at rotation | authored work owned by this PR |

So factor the body out and keep W2-300's enum, ancestry rule, and fail-closed
semantics byte-for-byte:

```rust
/// Commits reachable from `branch` but not from `cut`. `Unprovable` when the cut
/// cannot be placed on the branch — a rewritten branch or a missing object both
/// land here, because `is_ancestor` maps every nonzero exit to false.
fn commits_past(worktree: &Path, branch: &str, cut: &str) -> OpsResult<CommittedFollowUp>

fn committed_follow_up_range(worktree, settled) -> OpsResult<CommittedFollowUp>  // cut = head_sha (unchanged)
fn unpublished_work(worktree, pr) -> OpsResult<CommittedFollowUp>                // cut = base_commit
```

`unpublished_work`'s cut is never absent (`base_commit: String`, not `Option`),
so its only `Unprovable` is the non-ancestor arm: the branch was rewritten off its
recorded base, or the base object is gone. Both block. This is reuse, not a second
authority — there is exactly one classifier and one enum.

### 2. The gate classifies; only a completion acts

The gate gains one field. It is a *classification*, never a mutation:

```rust
pub(crate) struct CompletionGate {
    pub satisfied: bool,
    pub blockers: Vec<String>,
    /// An active successor that provably holds nothing: never published, and its
    /// branch never moved off the base recorded when it was minted.
    pub discardable_successor: Option<TaskPr>,
}
```

In the gate's active-PR arm, a `Working` PR that is `ProvenEmpty` **and** has a
merged predecessor is classified discardable and pushes no blocker. Every other
shape blocks and says which: a `Range` is authored work that would be stranded,
an `Unprovable` base means the branch cannot be *shown* empty (not the same as
being empty), and `ProvenEmpty` with nothing merged keeps today's refusal.

The scope says "lifecycle-created". I key on **emptiness, not provenance**: a
successor with no publication and no commits past its base holds nothing to lose,
whoever minted it, and a `created_by` column would be new durable state no other
decision reads. Endorsed on review.

### 3. Discard is part of completing, not a precondition for it

**The store already does this.** `complete_task_session(session, skipped_pr)`
(store/sqlite/child_sessions.rs:579) takes an optional PR, validates it is a
`Working` unpublished row on that Session, `DELETE`s it, and writes the terminal
status — in one `Immediate` transaction, guarded on
`publication_requested_at IS NULL AND merge_commit IS NULL AND abandoned_at IS NULL`.
It further refuses unless `session.status == Completed`. The capability was fully
built and had exactly one caller, `task_complete`, passing `None`. I nearly built a
second mechanism beside it; using it is keep-one-implementation, and it makes the
discard atomic with the completion rather than merely adjacent to it.

So `task_complete` becomes:

```rust
let gate = task_completion_gate(&store, &session).await?;
if !gate.satisfied {
    return Err(...);            // nothing written; the successor stays active
}
session.set_status(Completed, summary);
complete_task_session_with_authority(&store, &session, gate.discardable_successor.as_ref(), lease)?;
```

Ordering is the whole safety property. An earlier draft discarded *before* the
gate, as a precondition. That was wrong in a way that rebuilt the very loop this
Task removes: an unapproved review would then block **after** the successor was
already abandoned, leaving a non-terminal Task with no active PR — exactly the
state `ensure_working_pr_with_authority` rotates a fresh empty PR from. Because
the store refuses to skip a PR unless the Session is `Completed`, that mistake is
now unrepresentable through this path rather than merely avoided by discipline.

`advance_completion_after_gate` **declines** when a successor is discardable. It
completes through `complete_task_session_after_pr`, which settles the merged PR
and has no `skipped_pr`, so it cannot drop the row in the same transaction;
completing there would leave a terminal Task still holding an active PR. It is
unreachable on current rows anyway — a `CompleteTask` merge no longer rotates —
so `lf task complete` owns this shape, and there is one completion-decision path.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is #1050's tri-state actually on my base? | Yes — base is `0fef6d2ce`, "task: fail closed on committed follow-up past the merged tip (#1050)". `CommittedFollowUp` at ops/task.rs:3256. | No rebase or sequencing wait. The Task's "launch only once W2-300 settles" condition is met. |
| Does #1050 already stop the rotation that mints these? | Partly. `ensure_working_pr_with_authority` returns `Ok(None)` when the settled PR is `CompleteTask` and the carry is not a `Range` (ops/task.rs:3367). But W2-280's predecessor landed `--next`, so `after_merge == Review` and it rotates. | The empty successor is still reachable on the live path; the discard is not dead code for legacy rows only. |
| Does the auto path (`advance_completion_after_gate`) reach W2-280? | No. It needs `merged_completing_pr` — a merged PR with `after_merge == CompleteTask`. W2-280's is `Review`, so it returns false before the gate. | `lf task complete` is the settle path. The demo is that command. |
| Would abandoning really mint another PR? | Yes, and I read the code rather than trusting the memory entry. `abandon_task_pr` sets `Waiting` + "another PR may follow" (ops/task.rs:1761-1765); the next `ensure_working_pr` sees a settled PR and rotates. | The discard must settle the Task in the same operation, and must not reuse `abandon_task_pr`. |
| Can the discard strand uncommitted work? | No. `task_complete` already refuses a dirty worktree (ops/task.rs:3632) before anything I add. | No new dirty-tree check needed on that path; `advance_completion_after_gate` is guarded by `is_process_active` instead. |
| After the discard, does anything re-open the Task? | `repair_premature_completion` re-runs the gate. Post-discard: no active PR, and `prs.last()` is the abandoned successor whose phase is not `Merged`, so the merged-tip check is skipped. Gate satisfied → no repair. | Proof #4 holds by construction; pinned with a test rather than left to argument. |
| Does a primitive already exist for this? | **Yes, and it was dead.** `complete_task_session(session, skipped_pr)` deletes an unpublished `Working` PR and completes the Session in one transaction, refusing unless the Session is `Completed`. Its only caller passed `None`. | Use it instead of a second mechanism. The discard is atomic with completion, and the store — not caller discipline — enforces the ordering. |
| Is there a `PrAbandoned` event to emit? | No such variant in `TaskEventKind` (task/mod.rs:949-1035). | Do not invent one — a new event kind is a wire type mirrored in Rust and Swift. The discard is recorded in the completion `status_reason`, which flows into the existing `StatusChanged` event, plus `abandoned_at` on the row. |
| Can I write a real-runner regression, or only a mocked one? | #1050 shipped a real-git harness in the same module: `TestRepo` + `gate_task`, used by `completion_gate_blocks_when_follow_up_range_is_unprovable`. Successors are minted with `store.settle_task_pr(&merged, Some(&next))`. | Tests reproduce the shape against real git and the real store. No new abstraction, no factory. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Make `lf pr abandon` not mint a successor | Fixes the observed loop at its feed | Abandon is the operator saying "this PR was a mistake, give me another"; that rotation is its job. Removing it breaks the legitimate use to fix a case abandon should never have been asked to handle. |
| Never rotate after a merge until the body asks | Removes the empty successor at the source | Reshapes the serial-PR lifecycle wholesale, and strands the Tasks already sitting in this state. A settle-side discard settles W2-280 today; W2-300 is parked before its rotation and is not in this shape (see `scratch/questions.md`). |
| Record `created_by: lifecycle \| operator` on `TaskPr` | Matches the scope's "lifecycle-created" wording literally | New durable column that exactly one branch reads, and it answers the wrong question. An operator's `lf pr next` successor that is proven empty is equally safe to discard; a lifecycle successor holding commits is not. Emptiness is the fact that matters. |
| Let the gate itself discard | One call site instead of two | The gate is pure and is called from read-only paths (`task_status`, the repair). A mutating gate means `lf task status` silently changes PR state — and `repair_premature_completion` calls the gate on Completed sessions. |
| Discard as a precondition, before the gate (my first draft) | Reads simply: clear the artifact, then complete | **Rebuilds the loop inside its own fix.** Any other blocker — an open review — then refuses *after* the successor was abandoned, leaving a non-terminal Task with no active PR, which is precisely what rotates a fresh empty PR. Caught on review. |
| Abandon the row, then complete in a second write | Reuses `settle_task_pr` | Two writes have an instant between them. If the completion write fails, the Task is non-terminal with no active PR — the rotation shape again. The `skipped_pr` transaction has no such instant. |
| Treat a missing/rewritten base as empty | The successor "looks" empty | This is the exact fail-open #1050 closed on the read side. `is_ancestor` returns false for a missing object, so "can't see it" would read as "there's nothing there". `Unprovable` blocks. |

## Key decisions

- **Emptiness, not provenance, is the authority.** Justified above; the scope's
  "lifecycle-created" is a description of the observed shape, not the safety
  property.
- **One classifier, two cuts.** W2-300 owns the tri-state and the ancestry rule.
  I add a cut chooser, not a second opinion. If `commits_past` is wrong, both
  callers are wrong together — which is the point.
- **Discard does not reuse `abandon_task_pr`.** Its `Waiting` + "another PR may
  follow" transition is the defect being removed.
- **Requiring a merged predecessor.** A Task whose only PR is an empty
  unpublished sequence 1 keeps today's refusal. That case may well be a real gap
  — the CLI's own help says `lf task complete` "proposes completion for clean
  work that needs no PR", and today the gate blocks it — but it is not this
  Task's scope. Filed in `scratch/questions.md`.
- **No review-gate change.** `review_gate`, `required_reviews_for_task`, and
  eligibility are untouched. W2-297 owns the changes-requested deadlock. A Task
  with an unapproved required review still cannot complete after this lands — the
  discard clears the *PR* blocker and nothing else.

## Scope

**In scope**

- `commits_past` factored out of `committed_follow_up_range`; new
  `unpublished_work` cutting at `base_commit`.
- `settle_proven_empty_successor`, called from `task_complete` and
  `advance_completion_after_gate`.
- The gate's `Working` arm names the tri-state reason.
- Regression tests, including both sabotage directions.

**Out of scope**

- Review eligibility, gate policy, the changes-requested deadlock (W2-297).
- The committed-descendant classifier's own semantics (W2-300 / #1050).
- Rotation policy — when a successor is minted at all.
- `lf pr abandon`'s successor-minting behavior.
- A Task whose only PR is an empty unpublished sequence 1.
- New event kinds or DTO fields.

## Done when

```
cargo test -p loopflow --lib ops::task          # 67 pass; see note
cargo clippy -p loopflow --lib --tests -- -D warnings
cargo fmt --check
```

All green. Note: `recover_refuses_a_non_abandoned_task` and
`recover_abandoned_task_adopts_existing_worktree_pr_and_direction` fail *inside a
Task Session* on clean main — they lack the `AMBIENT_TASK_ENV` scrub, so the
Session's own `LF_WAVE_ID` leaks in. Proven not mine by stashing: they fail
identically with these changes removed. They pass in CI, which has no ambient Wave.

`task_complete` resolves the machine registry itself, so — like the existing gate
tests — these drive the two functions it composes: `task_completion_gate` and
`complete_task_session_with_authority`.

Five tests, each named for the fact it pins:

1. `a_merged_task_settles_over_a_proven_empty_successor` — the gate is satisfied
   and classifies the successor; evaluating it mutates nothing; the completion
   transaction leaves exactly 1 PR row (the merged one), no active PR, no
   replacement.
2. `a_pending_required_review_blocks_completion_and_keeps_the_successor` — the
   reviewer's required regression, and the one that pins the ordering. With the
   review open the gate refuses naming it, and the successor is **still Working**.
   It also asserts the store *refuses* to skip a PR outside a completing
   transaction — the invariant that makes a pre-gate discard unrepresentable. After
   approval, one completion settles it with no replacement minted.
3. `completion_is_withheld_over_work_committed_on_an_empty_successor` — `Range`:
   refuses, classifies nothing, and the commit is still reachable from the branch.
4. `completion_is_withheld_when_the_successor_base_is_unprovable` — the branch is
   rewritten off its recorded base; refuses, classifies nothing, row untouched.
5. `a_settled_task_stays_completed_across_reconciliation` — two reconcile passes
   after settling: status stays `Completed`, no `ReopenTask` writeback, no PR minted.

**Sabotage proofs**, both directions, run:

- Never classify a successor as discardable → tests 1, 2 and 5 go red, and test 1
  fails with today's exact refusal: *"the rotation's empty artifact must not block
  merged work: [\"pull request sequence 2 is unpublished; publish and merge it or
  run `lf pr abandon`\"]"*. The settle tests guard the fix, not the fixture.
- Ignore the tri-state and classify any `Working` successor → tests 3 and 4 go red.
  The guard is load-bearing, which test 1 alone cannot show — the failure this Task
  is most able to cause is discarding work, not refusing to.

## Measure

Baseline, from the live incident: W2-280 burned generation 8 for 1804s with no
durable progress, then relaunched generation 9 onto the same unanswerable gate.
ENG-7 produced two empty PRs and two full bodies on the same shape. After: a
merged Task with a proven-empty successor settles in one command, minting zero
bodies and zero PRs. The Developer Efficiency KR — "zero Sessions sit in failed
awaiting a manual resume" — cannot be true while this shape exists, because its
only exits today are a lie (publish empty work) or a loop (abandon).
