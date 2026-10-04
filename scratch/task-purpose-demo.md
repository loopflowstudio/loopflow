# Task purpose and completion demo · 2026-10-03

Interactive delivery review for LOO-367. Updated October 4: Jack Heart ended the
review and authorized a pursue pass to revise the code against the agreed design.
The next implementation is governed by the design's “Authorized revision” section.
The setup-only account below is historical; it does not describe all remaining
judgment. Changed behavior has not been demonstrated live to Jack Heart. This is
not acceptance of the current implementation or a navigation verdict. The saved
Flow's following loop-decide owns navigation using this feedback.

## Code walkthrough feedback · 2026-10-04

Jack Heart reviewed the [local code walkthrough](pr-review.html) and questioned
why conversations should gain Tasks without checkouts. Jack described an open-ended
Session becoming focused on a Task as the common path, then said: “I *think* its
better to default to having a worktree just in case they start editing stuff”.

Preferred direction: default that transition to a worktree ready for edits.
This is tentative product direction, not acceptance or a request to land. The
assistant's interpretation is to retain optional execution/delivery while separating
checkout creation from PR and Flow obligations. The exact handoff of the existing
conversation and any already-made edits remains unresolved; Jack has not selected
a restart, move, or transfer policy.

Next useful action: resolve that handoff and revise the default before treating the
branch's no-checkout association as the intended primary experience. The
[design](jack-heart/start-and-finish-tasks-without.md#working-product-direction--2026-10-04)
now records this direction above its historical implementation account. Proof must
show the same conversation and Task identity continuing with edits in the intended
worktree, preserving earlier history and edits, without requiring a Flow or delivery
promise merely to obtain the worktree. No such proof was run during this discussion.

### Completion clears a worktree without requiring landing

Jack Heart added: “and just make sure behavior is such taht if you open a worktree,
complete a task associated with that worktree, we clear the worktree, even if
nothing landed”. This is requested behavior, beyond the preceding tentative
preference about allocation defaults.

The design now requires cleanup after a research/no-change outcome without a
published or merged PR. A checkout alone creates no delivery promise. The current
allocation creates a first unpublished PR and the gate refuses it without a merged
predecessor, so the branch needs revision for this scenario. Existing tests that
seed a merge do not prove it. Implementation must also reconcile cleanup with the
continuing conversation and preserve actual unfinished work; no blanket discard
was requested. Review remains open, and no implementation or behavioral acceptance
is claimed by this note.

### Worker removal and visible completion · October 4

Jack Heart identified worker removal in another Task as relevant to item 7 and
expressed an expectation that the completion API is being removed. Fresh readback
identified [LOO-353](https://linear.app/loopflow/issue/LOO-353): remove all special
Task workers and privileged managed Flow selection, use ordinary background work,
and provide equally rich views of every associated Flow. The brief explicitly
removes `session ready`/`session complete`; it still assigns Task admission/completion
to LOO-367. Removal of `task complete` is not established by that brief.

Jack asked what the walkthrough's distinction among outcome, continuing conversation
and retained execution meant. The assistant clarified that the product concern is
the result of finishing work: understandable Task disposition, access to the same
conversation, and cleanup or a concrete retention reason. This is an explanation,
not an accepted new UI requirement. The design records the cross-Task dependency;
the earlier managed/independent split is not a final product contract.

### Confirmed experience · October 4

Jack Heart replied “yeah that all sounds right” to the clarification immediately
above and its concrete proposed result: the Task shows its outcome, the same
conversation remains available, the worktree is cleaned up even if nothing landed,
and an actual cleanup blocker is explained. The clarification proposed no separate
user-facing “retained execution” status. Jack also accepted the described direction
toward a standard display for all associated Flows under LOO-353.

This confirms the product direction discussed, not the branch implementation,
successful behavioral proof, review completion or delivery. The identified
Session-versus-Task completion API distinction does not itself decide a new API
removal. Next implementation planning must reconcile the default worktree path,
no-landing cleanup and conversation continuity with LOO-353's removal work.

## Earlier demo preparation · 2026-10-03

## Experience to review

The [current design](jack-heart/start-and-finish-tasks-without.md) describes a
research Task created, inspected, associated with a conversation and completed
without allocating a checkout, PR or Flow. The working conversation then reports
the result. Optional execution retains Task identity; Flow completion leaves the
Task open. Managed delivery still refuses completion while its promises remain
unresolved.

## Observations and limits

- The checkout was clean at review entry, at `c5dbcb72c`; implementation commits
  include `c423c9980`, `f656edf5f` and `25d339fb1`.
- `command -v lf` selected `/Users/jack/.local/bin/lf`; `lf --version` returned
  `lf 0.12.32`. The design identifies local base `8c72e591e` as v0.12.32.
  No installed candidate containing this branch was established.
- The branch adds `optional_task_workspace.sql`, which freezes retained Flow cwd
  and removes the placement-dependent validation triggers. It is absent from
  the active PR base.
- [TESTING.md](../TESTING.md) requires an explicit disposable `LF_HOME`, cleared
  inherited execution authority and a pinned source executable for branch CLI
  fixtures. Without that experiment, source commands forward to the installed
  CLI/main Home. Running `scripts/dev-lf` alone would not demonstrate this branch.
- No Task or external issue was created or completed, no installation was changed,
  and no Desktop interaction was observed. Existing focused-test results remain
  evidence with the limits recorded in the design; no suites were rerun here.

Interpretation: a configured candidate path is missing for the requested live
review. The installed command's availability does not establish the new behavior.
The Desktop build and its runtime selection were not inspected, so this note
does not claim a Desktop regression or a failed user interaction.

## Feedback and design

Jack Heart has supplied no new experiential feedback or acceptance in this review
yet. The October 2 decision permitting completion from the working conversation
remains in force. No design changes have been agreed or proposed by this review.
The blocker concerns demonstration setup and proof, not a new product question.

## Next useful action

Prepare a runnable candidate demonstration using a disposable Home with the
branch CLI and matching Desktop configured together. Preserve the current Home;
do not promote a draft-bearing binary to make this review possible. First finish
the design's missing CLI/provider/review/retry and headless Desktop acceptance
through the existing stateful fixtures. Those results must remain labeled as
fixture evidence.

Then guide Jack through the research lifecycle and managed-delivery refusal and
recovery, reading the resulting Task, Session and Flow state through their public
surfaces. Record the original outcome surviving retry, absence of invented
delivery objects, retained conversation history, and the conversation's response
after completion. If the intended review requires the installed Home specifically,
a schema-complete installed candidate is still required. Capture Jack's actual
observations and feedback here before claiming experiential acceptance.

Review may return this setup blocker as feedback; readiness does not complete the
review. Jack owns review completion and the following loop-decide owns navigation.

Check: `git diff --check` passed for this note; no implementation changed.
