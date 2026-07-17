---
description: Judge and advance one Task's PR after implementation.
default_agent: codex
action_style: procedural
---
Judge the Task change against its directive and design.

Inspect the worktree, focused verification, diff, PR, CI, and review state.

- If the change is incomplete or verification fails, state the next concrete
  repair; the Task runner may start another full flow iteration.
- If the diff is reviewable, publish or refresh it with `lf pr publish -c` when
  merge completes the Task, or `lf pr publish --next <slug>` when another serial
  PR follows.
- If CI or review has an obvious fix, make the bounded repair in this Task
  worktree, verify it, and update the same PR.
- Treat an open or approved PR as submitted, never complete. A merge settles
  that PR; it does not inherently complete the Task.
- Never use `pr open`, `submit`, or `land` as managed Task authority. Required
  review occurs in the provider-backed InteractionReview conversation; after
  every required review and current settlement condition clears, the runner
  lands mechanically. Use `lf pr abandon` to discard the active PR without
  abandoning the Task.
- Use `lf task complete <issue> --summary "..."` only for a clean Task that
  honestly needs no PR. Loopflow owns Linear completion and pending writeback.

Return concise evidence and the next external wait or repair. The Task runner
independently observes PR/worktree state to choose repeat, wait, block, or
complete; write no loop bit.
