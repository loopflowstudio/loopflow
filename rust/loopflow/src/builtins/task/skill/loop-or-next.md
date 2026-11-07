---
requires: objective, current results and feedback, and the caller's decision protocol
produces: Advance or Iterate with the reasons, or Blocked with what needs resolving
action_style: procedural
---
Decide whether the Graph should advance, iterate, or stop because progress is blocked.

Judge against the objective, success conditions and constraints the caller
supplies. Work and review steps bring their own criteria; this step decides
where the Graph goes.

Read the previous direction, what changed or was learned, current feedback and
open findings. Open the files they point to when those may have changed.
Accepted changes stand; a newer proposal doesn't override an accepted
requirement. A process exiting is not the work being done.

A check that can't run here, or needs a person, is never a reason for Blocked.
Use a headless equivalent or leave it to gate or CI and continue. Before gate,
a build and focused tests are enough. Real failures and missing behavior still
need work. Never claim a deferred check passed or skip an authored review.

- **Advance** when what this step requires is done, including acceptance
  conditions and earlier findings. Later obligations belong to later steps.
- **Iterate** when the last pass made real progress and specific work remains
  that the backward edge can do. Give the next action and the result it should
  produce, within the accepted scope. Ruling out a hypothesis or narrowing the
  problem counts as progress; activity doesn't.
- **Blocked** when the last pass made no real progress, repeated the same
  failure with nothing new learned, or needs an input or judgment you can't
  supply. Say what needs resolving. Don't spend another pass on unchanged
  direction. If results are missing, say so; missing results don't prove there
  was no progress.

Before deciding, answer separately: which obligations are met, what changed or
was learned, and what useful action or missing input remains. Look at actual
results, not a confidence score. There is no pass limit: decide from the work,
not the iteration count. The runner owns edge targets.

Answer through the supplied decision protocol for this exact decision. Advance
and Iterate move the Graph; Blocked stops it here. Existing output and logs are
enough; no separate handoff is needed.

On a retry, reassess with the new results or direction; a retry doesn't mean
the work is done. Authored reviews feed this assessment; they aren't verdicts.
Carry accepted changes, open questions and the next action into the summary,
with paths the next step can follow.

If output validation asks for a corrected value, correct it in this
conversation; don't rerun implementation. Malformed output doesn't mean no
progress. If the protocol still can't be satisfied, leave the failure visible.

Without a decision protocol, give the assessment in the conversation. Don't
make up a decision command, create a Task, launch the next step, or write a
file for a later agent to act on.
