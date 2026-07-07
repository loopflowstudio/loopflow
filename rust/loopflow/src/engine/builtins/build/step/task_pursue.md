---
description: Implement one pass of work toward a task PR.
default_agent: codex
action_style: procedural
---
Work the task PR.

## Orientation

Read the Linear task statement in `<lf:message>`, then read `scratch/<branch>.md` and `scratch/questions.md` if present. Follow the repo style guide.

## Work

- Implement the smallest coherent slice described by the design doc.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- Use `lf op pr` when the branch has a reviewable PR-shaped change.

Stay scoped to the Linear task. Put follow-up scope in `scratch/questions.md`.
