---
description: Submit the task PR when ready; otherwise leave honest state.
default_agent: codex
action_style: procedural
---
Mutate the task lifecycle honestly.

## Orientation

Read the Linear task statement in `<lf:message>`, inspect branch status, and check whether this task has an open PR.

## Work

- If the work is not PR-ready, write the blocker or next concrete action in `scratch/questions.md` and stop.
- If the work is PR-ready, run `lf op submit --create-pr` with clear PR copy when needed.
- If CI or review state is red and the fix is obvious, fix it and resubmit.

You decide the task is done by setting the bit, not by saying so: submit the PR and drive it to merged. The runner only reads GitHub back; "done" in your reply counts for nothing.
