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

- Acknowledge the seed's current directive before editing with the exact `lf
  task acknowledge` command it provides. Summarize how that direction changes
  the execution plan; provider acceptance alone is not incorporation.
- Own execution in this process and worktree. Operational Loopflow children such
  as `lf commit`, `lf pr publish`, `lf rebase`, and direct skill or flow calls are
  part of that execution and remain available. Do not boot a server, create a
  second Task Session, or delegate the task seed. If scoped PM reads fail,
  note the failure and continue from the seed rather than repairing auth.
- Delegate only bounded, independent checks through the execution tools already
  available to this process, and keep responsibility for integrating the result.
- Implement the smallest coherent slice described by the design doc.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- When progress requires supervisor judgment, run `lf task request-decision
  <issue-id> "question" --option "first" --option "second" --wait`. The Task's
  required Project Session resolves the routine choice or explicitly escalates
  it to the Wave. Do not invent a provider-specific approval path or start
  unrelated work while it is pending.
- Use `lf pr publish -c` when the published PR completes the Task, or `lf pr
  publish --next <slug>` when another serial PR follows. Publication records
  evidence and settlement intent but never arms merge. Do not use `pr open`,
  `submit`, or `land` in a managed Task; required review stays in the existing
  provider-backed InteractionReview conversation and the runner lands
  mechanically once every required checkpoint and current settlement condition
  clears.
- A merged PR settles that PR. Use `lf task complete <issue> --summary "..."` for clean
  work that needs no PR. Do not write Task completion directly through PM.
- If a PR merged out of band and follow-up work remains, `lf pr next [slug]` reconciles the merge and
  rotates to the next serial PR, carrying committed and uncommitted follow-up
  forward — no manual git surgery.
- File a concrete follow-up with `lf pm task create` when new work belongs later
  under a known project. Filing does not authorize launching it in this task.
- Report consequential progress through the Task Session; its linked events
  keep the owning Wave informed without copying raw tool chatter.

Stay scoped to the task. Put unresolved ambiguity in `scratch/questions.md`.
