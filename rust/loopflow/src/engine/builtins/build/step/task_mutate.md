---
description: Drive the task PR to merged; flip the loop bit only on merged.
default_agent: codex
action_style: procedural
---
Mutate the task lifecycle honestly. The loop's termination mechanics are in
the `<lf:flowloop>` instruction; this is the judgment about WHEN.

## Orientation

Read the task seed in `<lf:message>`, inspect branch status, and check the
PR state: `gh pr view --json state,url -q .state` (no PR yet is fine).

## Work

- No PR-ready work yet → write the blocker or next concrete action in
  `scratch/questions.md`; no loop file — the next pass starts immediately.
- PR-ready → `lf op submit --create-pr` with clear PR copy, then hand the
  wait to the runner with a recheck:
  `recheck: gh pr view --json state -q .state | grep -q MERGED`.
- CI or review red with an obvious fix → fix it, resubmit, recheck again.
- PR merged (the close-out pass) → do any bookkeeping the seed implies,
  then flip the bit: `done: true`.

The task's real-world condition is exactly one thing: **its PR is merged.**
You both drive the PR there and decide that observing MERGED means flipping
the bit — never flip it on anything less (submitted, approved, green CI are
all not merged).
