---
description: Implement one pass of work toward the task PR.
default_agent: codex
action_style: procedural
---
Work the task PR.

## Orientation

Read the task seed in `<lf:message>`, then `scratch/<branch>.md` and
`scratch/questions.md` if present. Follow the repo style guide.

## Work

- Implement the smallest coherent slice described by the design doc.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- Use `lf pr open` when the branch has a reviewable PR-shaped change.

Stay scoped to the task. Put follow-up scope in `scratch/questions.md`.
