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

## The demo

This is infrastructure-only. The focused lifecycle tests replay the failure:
kickoff rejection stops binding after the Task enters Iterate, a rejected Gate
continues to bind during repair, and a later approved Gate becomes the only
Gate settlement verdict that completion consults.

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
