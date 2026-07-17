# Open questions — W2-304

Assumptions taken in `scratch/settle-merged-work-over-a.md`, recorded rather than
escalated (headless run; each is defensible and reversible).

## Resolved by reading the tree

- **"Reuse the tri-state from W2-300 after it lands or rebases."** It already
  landed. Base commit `0fef6d2ce` is #1050; `CommittedFollowUp` is at
  ops/task.rs:3256. No wait, no rebase.
- **Which command settles W2-280?** `lf task complete`. The auto path
  (`advance_completion_after_gate`) needs a merged PR with `after_merge ==
  CompleteTask`; W2-280's predecessor landed `--next`, so its disposition is
  `Review` and the auto path returns false before the gate is even read.

- **W2-300 is not in this Task's covered shape, and this change does not settle
  it.** Corrected on review (ir_5ce3dc83). The launch directive said "two live
  Tasks are stranded over proven-empty successors right now"; that holds for
  W2-280 and not for W2-300, which has `active_pr = None` and exactly one PR row —
  sequence 1, merged, `after_merge = review`, not abandoned. It is parked *before*
  the rotation, not over a successor, so condition 1 of the discard ("an active PR
  exists and its phase is `Working`") can never hold and
  `settle_proven_empty_successor` returns false for it. W2-300 keeps today's
  behavior until it resumes and rotates a successor into the covered shape; only
  then does `lf task complete` settle it. **Do not widen scope to the no-successor
  shape** — rotation policy is out of scope by decision, and if the parked-before-
  rotation strand proves real it is a separate Task.

## Assumptions

- **Emptiness, not provenance, decides the discard.** The scope says
  "lifecycle-created successor". I did not add a provenance field: an operator's
  `lf pr next` successor that is proven empty is equally safe to discard, and a
  lifecycle successor holding commits is not. Provenance answers a different
  question than the one the safety property needs. A `created_by` column would be
  new durable state that exactly one branch reads.

- **A Task whose *only* PR is an empty unpublished sequence 1 keeps today's
  refusal.** The discard requires a merged predecessor, per the scope's "settle
  the authoritative merged predecessor". Worth noting as a probable separate gap:
  the CLI help says `lf task complete` "proposes completion for clean work that
  needs no PR", but the gate's `PrPhase::Working` arm blocks exactly that. If real,
  it deserves its own Task rather than a widened blast radius here.

- **The `skipped_pr` capability was already built and unused.** `complete_task_session`
  has taken an optional PR to delete-and-complete since before this Task; its only
  caller passed `None`. I found it only after writing a parallel
  `settle_proven_empty_successor`. Recorded because the near-miss is the point: the
  question "does a primitive for this exist?" has to be asked against the *store*,
  not just the ops module I was standing in.

- **A dirty worktree is not re-checked in the discard.** `task_complete` already
  refuses an unclean worktree at ops/task.rs:3632, before anything I add, and
  `advance_completion_after_gate` refuses while the body is process-active. So the
  discard cannot strand uncommitted edits without a second check. If a third
  caller is ever added, it must carry one of those two guards.

- **`advance_completion_after_gate` declines a discardable successor rather than
  handling it.** Its store primitive (`complete_task_session_after_pr`) settles the
  merged PR and has no `skipped_pr`, so it cannot drop the row atomically;
  completing there would leave a terminal Task holding an active PR. Declining
  preserves today's behavior exactly, and the path is unreachable on current rows
  anyway (a `CompleteTask` merge no longer rotates). If a future change makes a
  `CompleteTask` merge rotate again, that path needs the same transaction, not a
  second discard.

- **`rev_parse` failure stays a hard error, not `Unprovable`.** A missing branch
  makes `commits_past` error out of the gate, which means `lf task status` errors
  rather than reporting a blocker. That is #1050's existing behavior for the merged
  cut and I kept the shared body faithful to it rather than changing semantics W2-300
  owns. Arguably both should be `Unprovable` — fail closed and stay readable. If it
  bites, it is a small follow-up against the one shared function.

- **After the discard, `prs.last()` is the abandoned successor, so the gate stops
  scanning the merged predecessor's post-merge follow-up.** Safe in every path I can
  construct: rotation cherry-picks the predecessor's follow-up range onto the
  successor, so follow-up present at rotation time shows up as `Range` on the
  successor and blocks. Work committed onto the predecessor's branch *after* the
  worktree rotated away from it is not a path the runner produces. Named here because
  it is the one place this design narrows what the gate looks at.
