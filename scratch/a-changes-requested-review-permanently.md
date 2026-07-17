# Changes-requested review lifecycle

## Problem

`required_reviews_for_task` treats every historical required review as a
standing completion verdict. A kickoff review completed with
`changes_requested` therefore bars completion forever, even after the Task has
left kickoff and repaired the design. The same all-history rule means a later
approved Gate cannot supersede an earlier rejected Gate.

The review rows are correct immutable audit evidence. The defect is only the
completion gate's eligibility rule: it confuses historical dispositions with
the review epochs that still govern the Task.

## User-visible outcome

A reviewer can request changes without permanently killing the Task. After the
Task leaves kickoff, its rejected kickoff review stops blocking completion. A
rejected Gate continues to block while the Task repairs in Iterate, and approval
from the next Gate supersedes that rejection so the existing Task can complete.

## End-to-end proof

Replay one Task Session through the authoritative lifecycle and review store:

1. Complete a required kickoff review with `changes_requested`, enter Iterate,
   and observe that `task_completion_gate` no longer reports that review.
2. Complete a required Gate review with `changes_requested`, enter Iterate, and
   observe that the same review still blocks completion during repair.
3. Enter the next Gate, complete its required review with `approved`, and
   observe that `task_completion_gate` is satisfied while both immutable review
   rows remain stored.

The three focused `ops::task` tests prove these boundaries through the real
SQLite store and completion-gate consumer. Run them with `cargo test -p
loopflow --lib <test-name>` using the names under Done when.

## Source of truth

The persisted `interaction_reviews` rows are authoritative review evidence:
`task_session_id`, `policy`, `phase`, `phase_epoch`, `status`, and `disposition`
do not change meaning. `TaskSession.lifecycle_phase` and `phase_epoch` are the
authoritative current lifecycle coordinate. `required_reviews_for_task` derives
the still-binding projection from those records; `review_gate` and
`task_completion_gate` consume that projection. No review row is deleted or
re-disposed.

## Affected surfaces and consumers

- `required_reviews_for_task` changes which persisted reviews still govern the
  current completion attempt.
- `review_gate`, `task_completion_gate`, completion reconciliation, and the
  existing `lf task complete` path inherit that projection without API changes.
- Interaction-review creation, messaging, completion, and audit reads remain
  compatible. SQLite schema, wire DTOs, CLI syntax, apps, Task runner behavior,
  and PR automation do not change.

## Absent and error states

- No required reviews means review eligibility contributes no blocker.
- No required Gate review means only required reviews at the current epoch are
  eligible.
- Every required Gate review tied at the newest Gate epoch remains eligible;
  all must satisfy the existing approval rule.
- Reviews from another Task Session and `Defer` reviews remain excluded.
- A requested, active, incomplete, or non-approved eligible review keeps the
  existing named completion blocker.
- Failure to list interaction reviews remains a completion-gate error; the
  projection does not invent an empty history or weaken the gate.

## Operational boundary

Keep the existing single interaction-review store read. Compute one in-memory
maximum and one filter over that result: linear time in the Session review
history, no extra query, network request, subprocess, retry, or recovery path.

## Approach

Keep required reviews eligible when either:

- the review belongs to the Session's current phase epoch, preserving the live
  kickoff or Gate waitpoint; or
- the review belongs to the newest required Gate epoch, preserving the latest
  settlement verdict while the Task repairs in Iterate.

Calculate the newest required Gate epoch locally inside
`required_reviews_for_task`, then apply those two eligibility arms after the
existing Session and `Require` policy filters. Older reviews remain stored and
queryable but stop acting as permanent blockers.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Must a completed review be mutated to repair the lifecycle? | No. Completed dispositions are intentionally write-once audit evidence. | Leave review persistence unchanged and scope only the completion read. |
| Are Gate cycles ordered by the stored epoch? | Yes. `TaskSession::enter_iterate` and `TaskSession::enter_gate` each accept only their adjacent lifecycle edge and increment `phase_epoch` once. `task_lifecycle_repeats_iterate_and_gate_until_approval` exercises kickoff epoch 1 through Iterate epoch 2, Gate epoch 3, and the next Iterate epoch 4. | The maximum required Gate epoch identifies the latest settlement cycle without new state. |
| Is the current epoch alone enough? | No. After a rejected Gate returns the Task to Iterate, the Gate epoch is no longer current but its rejection must still bar completion. | Retain the newest required Gate epoch alongside the current epoch. |
| Can all Gate history remain binding? | No. A later approved Gate must settle the repaired Task without the earlier rejection remaining immortal. | Select only the maximum required Gate epoch. |
| Does this require a new lifecycle state? | No. Reviews already carry phase and phase epoch, and the Session carries its current epoch. | Derive eligibility from existing data with no schema or state transition. |
| Should the dead-Task burn be repaired here? | No. Runner and PR-rotation changes overlap W2-300 committed-follow-up rotation and W2-290 newest-PR action ownership. | Make no Task runner, PR rotation, or merged-PR changes in this Task. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Allow a completed `changes_requested` review to be re-disposed | Reuses one row but destroys the write-once audit model. | The stored review is truthful; only its continuing gate eligibility is wrong. |
| Consult only reviews from the current phase epoch | Minimal filter, but a rejected Gate stops binding as soon as repair begins in Iterate. | It permits completion before the repaired work passes a later Gate. |
| Consult only the newest Gate epoch | Correct after Gate begins, but drops a required kickoff review before any Gate exists. | The live current-epoch waitpoint must remain binding. |
| Park or rotate Tasks differently after completion refusal | Could address the downstream burn. | It changes runner and PR ownership outside this defect and conflicts with W2-300/W2-290. |

## Key decisions

- Treat review history as immutable evidence and review eligibility as a derived
  projection.
- Keep the current epoch and newest Gate epoch as the only two eligibility
  coordinates.
- Calculate the newest Gate epoch locally; add no helper, field, or lifecycle
  state.
- Prove the two eligibility arms and Gate supersession directly with three
  focused gate tests.

## Scope

- In scope: `required_reviews_for_task` eligibility and the three lifecycle
  regression tests.
- Out of scope: review mutation, Task runner behavior, completion wait reasons,
  merged-PR detection, `ensure_working_pr`, `pr next`, and all PR rotation.

## Done when

- `a_superseded_kickoff_changes_request_does_not_bar_completion` passes.
- `a_rejected_gate_cycle_still_bars_completion_from_iterate` passes.
- `the_newest_gate_cycle_supersedes_an_earlier_changes_request` passes.
- Removing current-epoch eligibility breaks the kickoff proof; removing newest
  Gate retention breaks the repair proof; retaining all Gate history breaks the
  supersession proof.
- `cargo fmt --check` and
  `cargo clippy -p loopflow --lib --tests -- -D warnings` pass.
- Production changes remain below 30 added lines, with no Task runner or PR
  rotation diff.
