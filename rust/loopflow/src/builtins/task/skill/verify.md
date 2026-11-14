---
requires: A merged PR and the Task's Done when claims
produces: a verdict on the merged change with what was checked, a fix forward, or follow-up Tasks
action_style: conversational
---
Check the merged change where people will meet it, then finish the Task or fix forward.

Read the Task's outcome and Done when claims, the merged PR and any demo notes
in `scratch/`. Check each claim against what main does now, not the branch you
remember. If nothing is left to check after merge, say why and finish.

Inspect post-merge CI for the exact merge SHA. Report its event, run link and
result separately from PR and merge-queue checks; a green PR or queue does not
establish a green push run. If the push run is pending or unobserved, say so.

Check headless first; most claims need no person:

- Build main and run its `lf` with its own `LF_HOME` so nothing touches real
  work; unset inherited `LF_*` variables first.
- Seed a fake world when the claim needs Tasks, Waves or Sessions, and drive the
  real commands against it.
- For the Mac app, build it and run its view and interaction tests headless;
  capture screenshots headless when a view's look is the claim.
- After a release, run the installed binary the way people do.

A claim you can't check is a finding, not a hand-back. Name what was missing,
and build the missing route as part of a fix when it's small, or capture it as a
follow-up Task.

Pick one outcome and record it, with the commands and what they printed, in a
topic-named note in `scratch/` and in your final message:

- **Verified.** Every claim holds on main. Advance; the Task completes.
- **Fix forward.** A promised claim doesn't hold. Name the gap and iterate: the
  fix continues in this Task's checkout from main and ships as a new PR, keeping
  the merged one as history. Never revert.
- **Follow-up.** Something outside this Task's outcome is wrong or missing.
  Create a linked follow-up with `lf task create`, then advance.

With someone in the conversation, show what you checked and let them judge the
experience. Headless, decide and keep moving; there's no one to hand a blocker to.
