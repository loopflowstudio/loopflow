# Files demo: reading-first Files and following scratch pages

Reviewed 2026-10-25 with Jack in a separate app bundle, **Loopflow Dev 581**,
built from this branch. Design: `scratch/explore-step.md`.

## Observed and accepted

- `scratch/explore-step.md` opens rendered in **Read**; Read / Edit / Diff
  replace the old File / Diff picker. Jack: "ok, worked".
- A scratch HTML page written while Jack had just used Files arrived as an
  unselected tab and rendered as a styled page when clicked. Jack: "yep, worked".
- After the fix below, a scratch Markdown page written after 2 idle minutes
  took the view without a click. Jack: "yep!"

## Changed during the demo

- First attempt at focus-taking failed: the page stayed a background tab.
  Cause: the rule refused while Files held pane focus, which Files keeps until
  you click elsewhere, so an idle pane never counted as stale. The clause is
  deleted; idle time, Edit/Diff mode and unsaved drafts decide.
  `scripts/test_desktop.sh --filter TaskFilesTests` → 26 passed.

## Not confirmed

- Scroll position holding across an HTML rewrite: the rewrite ran, but Jack
  did not report on the position.

## Found along the way (not this Task)

- One `~/Applications/Loopflow Dev.app` slot: another worktree's
  `loopflow-dev.py run` replaced this build mid-demo. Worked around with a
  second bundle id.
- A Task with no Wave never appears in the Desktop sidebar (LOO-581 had to be
  moved to product).
- Dev builds on a store copy show `UserPromptSubmit`/`PostToolUse` hook errors
  in every Session ("working on the copy…"), and the copy never sees shared
  store changes.
