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

- **A dirty worktree is not re-checked in the discard.** `task_complete` already
  refuses an unclean worktree at ops/task.rs:3632, before anything I add, and
  `advance_completion_after_gate` refuses while the body is process-active. So the
  discard cannot strand uncommitted edits without a second check. If a third
  caller is ever added, it must carry one of those two guards.

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
