---
requires: A merged PR and the Task's Done when claims
produces: a verdict on the merged change with what was checked, a fix forward, or follow-up Tasks
action_style: conversational
---
Check the merged change where people will meet it, then finish the Task or fix forward.

Read the Task's outcome and "Done when" claims, the merged PR and any demo
notes. Check each claim against what main does now, not the branch you
remember. If nothing is left to check after merge, say why and finish.

Read post-merge CI for the exact merge SHA. A green PR or merge queue says
nothing about the push run; report it separately, and say so if it is pending.

Skip rebuilding `main` when all three hold:

- Every shipped file on `main` matches the tree `qa` exercised:
  `git diff <qa head> <merge SHA> -- . ':!scratch'` prints nothing (where
  `<qa head>` is the branch's `lf qa:` commit, and `scratch/` is excluded
  because `lf land` clears added scratch files before merging).
- Post-merge CI for `<merge SHA>` is green.
- No claim depends on something that only exists after merge (a release
  install, a deployment, production state).

When all three hold, verify from the `qa` record and the CI run,
and cite them. Any missing—or any diff outside `scratch/`, including edits by
`land` or synced upstream commits—means the full check below.

Check headless first; most claims need no person. Build main and run it
isolated from real work. Seed the state a claim needs and drive the real
commands against it. Run apps through their headless view and interaction
tests. After a release, run the installed build the way people do.

A claim you can't check is a finding, not a hand-back. Name what was missing,
and build the missing route as part of a fix when it's small, or capture it as
a follow-up.

Pick one outcome and record it, with the commands and what they printed, in a
note under `scratch/` and in your final message:

- **Verified.** Every claim holds on main. Advance; the Task completes.
- **Fix forward.** A promised claim doesn't hold. Name the gap and iterate: the
  fix continues in this Task's checkout from main and ships as a new PR. Never
  revert.
- **Follow-up.** Something outside this Task's outcome is wrong or missing.
  Create a linked follow-up with `lf task create`, then advance.

With someone in the conversation, show what you checked and let them judge the
experience. Headless, decide and keep moving.
