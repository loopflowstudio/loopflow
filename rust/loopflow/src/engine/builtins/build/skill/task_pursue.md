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

- Honor every Steer included in the seed and summarize how it changes the
  execution plan. The boundary Basis is fixed; provider acceptance alone is
  not application.
- Own execution in this process and worktree. Operational Loopflow children such
  as `lf commit`, `lf pr land`, `lf rebase`, and direct skill or flow calls are
  part of that execution and remain available. Do not boot a server, create a
  second Task Session, or delegate the task seed. If scoped PM reads fail,
  note the failure and continue from the seed rather than repairing auth.
- Delegate only bounded, independent checks through the execution tools already
  available to this process, and keep responsibility for integrating the result.
- Implement the smallest coherent slice described by the design doc.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- When progress requires supervisor judgment, state the exact question and
  alternatives in the interactive step, then stop dependent work. The derived
  Review routes the Turn to the parent; do not invent a provider-specific
  decision command or start unrelated work while it is pending.
- Use `lf pr publish` when the branch has a reviewable PR-shaped change; it
  pushes and creates or refreshes the PR without opening a browser. Reach for
  `lf pr open` only when a human explicitly asked to see the PR for review.
- A merged PR settles that PR. Use `lf pr land -c` only when that merge
  completes the Task; use bare `lf pr land --next <slug>` when another serial
  PR follows. Use `lf task complete <issue> --summary "..."` for clean
  work that needs no PR. Do not write Task completion directly through PM.
- If a PR merged out of band (GitHub auto-merge, not settled by `lf pr land
  -c`) and follow-up work remains, `lf pr next [slug]` reconciles the merge and
  rotates to the next serial PR, carrying committed and uncommitted follow-up
  forward — no manual git surgery.
- File a concrete follow-up with `lf pm task create` when new work belongs later
  under a known project. Filing does not authorize launching it in this task.
- Report consequential progress through the Task Session; its linked events
  keep the owning Wave informed without copying raw tool chatter.

Stay scoped to the task. Put unresolved ambiguity in `scratch/questions.md`.
