---
description: Turn a task seed into a computable design doc for one small PR.
default_agent: codex
action_style: procedural
---
Clarify the task into one small PR's worth of work.

## Orientation

The task seed is in `<lf:message>` — free text: a sentence, an error paste, a
file path, sometimes a tracker reference. Read it, then read `scratch/` and
the repo style guide. This flowloop owns one design doc and one PR; the run
row is the task's only tracking — there is no backlog item to maintain.

## Work

- If `scratch/<branch>.md` already gives a clear build plan, leave it alone.
- If the design doc is missing or too vague to compute from, write the
  smallest useful `scratch/<branch>.md`.
- If the seed references something external (a tracker id, a URL) read it if
  you cheaply can; if auth fails, work from the seed text alone and note it.
- Record genuine ambiguity in `scratch/questions.md`, choose the simpler
  path, and keep moving.

Do not implement product code in this phase unless the clarification is
trivial and directly unblocks the next phase.
