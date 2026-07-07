---
description: Make a Linear task design doc computable for the task flowloop.
default_agent: codex
action_style: procedural
---
Clarify the task artifact for one small PR.

## Orientation

Read the Linear task statement in `<lf:message>`, then read `scratch/` and the repo style guide. The task flowloop owns one design doc and one PR.

## Work

- If `scratch/<branch>.md` already gives a clear build plan, leave it alone.
- If the design doc is missing or too vague to compute from, write the smallest useful `scratch/<branch>.md`.
- Record genuine ambiguity in `scratch/questions.md`, choose the simpler path, and keep moving.

Do not implement product code in this phase unless the clarification is trivial and directly unblocks the next phase.
