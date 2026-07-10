---
description: Implement one pass of work toward the task PR.
default_agent: codex
action_style: procedural
---
Work the task PR.

## Orientation

Read the task seed in `<lf:message>`, then `scratch/<branch>.md` and
`scratch/questions.md` if present. If the seed names a filed task, read that
record. Inspect related or in-flight work when it can reveal conflicts,
dependencies, or reusable context; do not turn task execution into backlog
selection. Follow the repo style guide.

## Work

- Own execution in this process and worktree. Operational Loopflow children such
  as `lf commit`, `lf pr land`, `lf rebase`, and direct skill or flow calls are
  part of that execution and remain available. Never invoke `lf loop` from a
  task. Do not boot a server or delegate the task seed. If scoped PM reads fail,
  note the failure and continue from the seed rather than repairing auth.
- Delegate only bounded, independent checks through the execution tools already
  available to this process, and keep responsibility for integrating the result.
- Implement the smallest coherent slice described by the design doc.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- Use `lf pr open` when the branch has a reviewable PR-shaped change.
- When a filed task id is known and its PR ships, close it with
  `lf pm task done --id <task-id> --pr <url>`.
- File a concrete follow-up with `lf pm task create` when new work belongs later
  under a known project. Filing does not authorize launching it in this task.
- At a detached pass boundary, report concrete progress with `lf radio pub` and
  publish durable learnings with `lf memory add`; the vendor conversation is
  private.

Stay scoped to the task. Put unresolved ambiguity in `scratch/questions.md`.
