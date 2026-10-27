---
requires: A merged PR and the Task's Done when claims
produces: a verdict on the merged change, with evidence, a fix forward, or follow-up Tasks
action_style: conversational
---
Check the merged change where people will meet it, then finish the Task or fix forward.

Read the Task's outcome and Done when claims, the merged PR and any demo notes
under `scratch/`. Verify each claim against what main now does, not against the
branch you remember. Some Tasks have nothing more to verify after merge; say so
with the reason and finish.

Verify headless first. Most claims have a route that needs no person:

- Build main and run `lf` from it with its own `LF_HOME`, so nothing touches
  real work; unset inherited `LF_*` variables first.
- Seed a fake world when the claim needs Tasks, Waves or Sessions to look at,
  and drive the real commands against it.
- For the Mac app, build it and run its view and interaction tests headless;
  capture screenshots with headless tooling when a view's look is the claim.
- After a release, run the installed binary the way a person would.

When you cannot verify a claim, that is a finding, not a hand-back. Name what
was missing. Build the missing route as part of a fix when it is small, or
capture it as a follow-up Task.

Decide one of three outcomes and record it with its evidence in a topic-named
note under `scratch/` and in your final message:

- **Verified.** Every claim holds on main. Advance; the Task completes.
- **Fix forward.** A claim the Task promised does not hold. Name the gap and
  iterate: the fix continues in this Task's checkout from main and delivers as
  a new PR, keeping the merged one as history. Never revert.
- **Follow-up.** Something outside this Task's outcome is wrong or missing.
  Capture a linked follow-up Task with `lf task create`, then advance.

With a person present, show them what you checked and let them judge the
experience. Headless, decide as they would and keep moving; there is no one to
hand a blocker to.
